import os
import shutil
import subprocess
import tempfile

from test_framework.blockchain_node import BlockChainNodeType, BlockchainNode
from utility.utils import (
    arrange_port,
    blockchain_rpc_port,
    blockchain_rpc_port_core,
    wait_until,
)
from utility.simple_rpc_proxy import SimpleRpcProxy


def _chain_data_dir() -> str:
    return os.path.join("tmp", f"data_{blockchain_rpc_port(0)}")


def _chain_make_args(root_dir: str, target: str) -> list[str]:
    data_dir = _chain_data_dir()
    return [
        "make",
        target,
        f"DATA_DIR={data_dir}",
        f"ETH_HTTP_PORT={blockchain_rpc_port(0)}",
        f"ETH_WS_PORT={arrange_port(1, 0)}",
        f"ETH_METRICS_PORT={arrange_port(2, 0)}",
        f"AUTHRPC_PORT={arrange_port(3, 0)}",
        f"CONSENSUS_RPC_PORT={blockchain_rpc_port_core(0)}",
        f"CONSENSUS_P2P_PORT={arrange_port(7, 0)}",
        f"NODE_API_PORT={arrange_port(4, 0)}",
        f"P2P_PORT={arrange_port(5, 0)}",
        f"DISCOVERY_PORT={arrange_port(6, 0)}",
    ]


def zg_node_init_genesis(binary: str, root_dir: str, num_nodes: int):
    assert num_nodes == 1, "Makefile deploy only supports one blockchain node"

    tests_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    os.environ["ZGS_BLOCKCHAIN_RPC_ENDPOINT"] = f"http://127.0.0.1:{blockchain_rpc_port(0)}"

    log_file = tempfile.NamedTemporaryFile(
        dir=root_dir, delete=False, prefix="init_genesis_", suffix=".log"
    )
    ret = subprocess.run(
        args=_chain_make_args(root_dir, "deploy"),
        cwd=tests_dir,
        stdout=log_file,
        stderr=log_file,
    )
    log_file.close()

    assert ret.returncode == 0, (
        "Failed to deploy 0gchain genesis, see more details in log file: %s"
        % log_file.name
    )


class ZGNode(BlockchainNode):
    def __init__(
        self,
        index,
        root_dir,
        binary,
        updated_config,
        contract_path,
        log,
        rpc_timeout=10,
    ):
        assert index == 0, "Makefile start only supports one blockchain node"

        self._root_dir = root_dir
        os.environ.setdefault(
            "ZGS_BLOCKCHAIN_RPC_ENDPOINT",
            f"http://127.0.0.1:{blockchain_rpc_port(0)}",
        )
        data_dir = os.path.join(root_dir, "0gchaind", "node" + str(index))
        rpc_url = os.environ.get(
            "ZGS_BLOCKCHAIN_RPC_ENDPOINT", "http://127.0.0.1:8545"
        )
        self._make_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

        super().__init__(
            index,
            data_dir,
            rpc_url,
            binary,
            {},
            contract_path,
            log,
            BlockChainNodeType.ZG,
            rpc_timeout,
        )

    def setup_config(self):
        """Already initialized by Makefile deploy"""

    def start(self):
        self.log.info("Starting 0gchaind via Makefile")
        ret = subprocess.run(
            args=_chain_make_args(self._root_dir, "start"),
            cwd=self._make_dir,
        )
        assert ret.returncode == 0, "Failed to start 0gchaind via Makefile"
        self.running = True

    def stop(self, expected_stderr="", kill=False, wait=True):
        ret = subprocess.run(
            args=_chain_make_args(self._root_dir, "stop"),
            cwd=self._make_dir,
        )
        assert ret.returncode == 0, "Failed to stop 0gchaind via Makefile"
        shutil.rmtree(os.path.join(self._make_dir, _chain_data_dir()), ignore_errors=True)
        self.running = False

    def wait_for_rpc_connection(self):
        rpc = SimpleRpcProxy(self.rpc_url, timeout=self.rpc_timeout)

        def check():
            return rpc.eth_syncing() is False

        wait_until(check, timeout=self.rpc_timeout)
        self.rpc_connected = True
        self.rpc = rpc
