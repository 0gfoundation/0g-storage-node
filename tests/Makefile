# Local Single-Node Testnet Makefile
# Usage: make [target]
# Steps: make build -> make deploy -> make start

include config/.env
export

SHELL := /bin/bash
ROOT_DIR := $(CURDIR)
CONFIG_DIR := $(ROOT_DIR)/config
BIN_DIR ?= $(ROOT_DIR)/bin
DATA_DIR ?= $(ROOT_DIR)/data
SRC_DIR ?= $(ROOT_DIR)/src
TMP_DIR ?= $(ROOT_DIR)/tmp
GETH_BIN := $(TMP_DIR)/geth
CHAIND_BIN := $(TMP_DIR)/0gchaind

ETH_HTTP_PORT ?= 8545
ETH_WS_PORT ?= 8546
ETH_METRICS_PORT ?= 9001
AUTHRPC_PORT ?= 8551
CONSENSUS_RPC_PORT ?= 26657
NODE_API_PORT ?= 3500
CONSENSUS_P2P_PORT ?= 26656
P2P_PORT ?= 30303
DISCOVERY_PORT ?= 30303

# Colors
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m

.PHONY: help clone build deploy start stop restart status logs clean all

help:
	@printf "$(GREEN)Local Single-Node Testnet$(NC)\n"
	@printf "\n"
	@printf "$(YELLOW)Quick Start:$(NC)\n"
	@printf "  all             - Clone, build, deploy, and start (full setup)\n"
	@printf "\n"
	@printf "$(YELLOW)Step-by-step:$(NC)\n"
	@printf "  clone           - Clone 0g-chain-v2 and 0g-reth repositories\n"
	@printf "  build           - Build 0gchaind and reth binaries\n"
	@printf "  deploy          - Generate genesis and initialize node data\n"
	@printf "  start           - Start the node (reth + 0gchaind)\n"
	@printf "\n"
	@printf "$(YELLOW)Control:$(NC)\n"
	@printf "  stop            - Stop all processes\n"
	@printf "  restart         - Stop and start again\n"
	@printf "  clean           - Remove all generated data (keep binaries)\n"
	@printf "  clean-all       - Remove everything including binaries and source\n"
	@printf "\n"
	@printf "$(YELLOW)Monitoring:$(NC)\n"
	@printf "  status          - Check node status\n"
	@printf "  logs            - Tail consensus layer logs\n"
	@printf "  logs-reth       - Tail execution layer logs\n"

#
# ============== ALL-IN-ONE ==============
#

all: clone build deploy start
	@printf "$(GREEN)All done! Node is running.$(NC)\n"
	@printf "$(YELLOW)RPC Endpoints:$(NC)\n"
	@printf "  Cosmos RPC:     http://localhost:$(CONSENSUS_RPC_PORT)\n"
	@printf "  ETH HTTP RPC:   http://localhost:$(ETH_HTTP_PORT)\n"
	@printf "  ETH WS RPC:     ws://localhost:$(ETH_WS_PORT)\n"

#
# ============== CLONE ==============
#

clone:
	@printf "$(GREEN)Cloning repositories...$(NC)\n"
	@mkdir -p $(SRC_DIR)
	@if [ ! -d "$(SRC_DIR)/0g-chain-v2" ]; then \
		printf "$(YELLOW)Cloning 0g-chain-v2...$(NC)\n"; \
		git clone $(CHAIN_REPO) $(SRC_DIR)/0g-chain-v2; \
		cd $(SRC_DIR)/0g-chain-v2 && git checkout $(CHAIN_BRANCH); \
	else \
		printf "$(YELLOW)0g-chain-v2 already exists, skipping...$(NC)\n"; \
	fi
	@if [ ! -d "$(SRC_DIR)/0g-reth" ]; then \
		printf "$(YELLOW)Cloning 0g-reth...$(NC)\n"; \
		git clone $(RETH_REPO) $(SRC_DIR)/0g-reth; \
		cd $(SRC_DIR)/0g-reth && git checkout $(RETH_BRANCH); \
	else \
		printf "$(YELLOW)0g-reth already exists, skipping...$(NC)\n"; \
	fi
	@printf "$(GREEN)Clone complete!$(NC)\n"

#
# ============== BUILD ==============
#

build: build-chain build-reth
	@printf "$(GREEN)Build complete!$(NC)\n"

build-chain:
	@printf "$(GREEN)Building 0gchaind...$(NC)\n"
	@mkdir -p $(BIN_DIR)
	@cd $(SRC_DIR)/0g-chain-v2 && git checkout dev-v2.1 && make build
	@cp $(SRC_DIR)/0g-chain-v2/build/bin/0gchaind $(BIN_DIR)/0gchaind
	@printf "$(GREEN)0gchaind built successfully!$(NC)\n"

build-reth:
	@printf "$(GREEN)Building reth...$(NC)\n"
	@mkdir -p $(BIN_DIR)
	@cd $(SRC_DIR)/0g-reth && cargo build --release
	@cp $(SRC_DIR)/0g-reth/target/release/reth $(BIN_DIR)/
	@printf "$(GREEN)reth built successfully!$(NC)\n"

#
# ============== DEPLOY ==============
#

deploy:
	@printf "$(GREEN)Initializing node data...$(NC)\n"
	@mkdir -p $(DATA_DIR)/.tmp
	@chmod +x $(GETH_BIN) $(CHAIND_BIN)
	
	@# Initialize reth
	@printf "$(YELLOW)Initializing reth...$(NC)\n"
	@rm -rf $(DATA_DIR)/.tmp/eth-home
	@$(GETH_BIN) init --datadir $(DATA_DIR)/.tmp/eth-home --chain $(CONFIG_DIR)/eth-genesis.json
	@mkdir -p $(DATA_DIR)/.tmp/eth-home/geth
	@cat $(CONFIG_DIR)/node0key > $(DATA_DIR)/.tmp/eth-home/geth/nodekey
	
	@# Initialize 0gchaind
	@printf "$(YELLOW)Initializing 0gchaind...$(NC)\n"
	@rm -rf $(DATA_DIR)/.tmp/0gchaind
	@$(CHAIND_BIN) init $(MONIKER) --chain-id $(CHAINID) \
		--chaincfg.chain-spec=$(CHAIN_SPEC) --chaincfg.chain-spec-file=$(CONFIG_DIR)/spec.toml \
		--home $(DATA_DIR)/.tmp/0gchaind
	
	@# Copy node keys
	@cat $(CONFIG_DIR)/node0_key.json > $(DATA_DIR)/.tmp/0gchaind/config/node_key.json
	@cat $(CONFIG_DIR)/priv_validator0_key.json > $(DATA_DIR)/.tmp/0gchaind/config/priv_validator_key.json
	
	@# Add premined deposit
	@printf "$(YELLOW)Adding premined deposit...$(NC)\n"
	@$(CHAIND_BIN) genesis add-premined-deposit \
		--chaincfg.chain-spec=$(CHAIN_SPEC) --chaincfg.chain-spec-file=$(CONFIG_DIR)/spec.toml \
		--home $(DATA_DIR)/.tmp/0gchaind 1000000000000000 $(DEPOSITOR0)
	
	@# Collect deposits and generate genesis
	@printf "$(YELLOW)Collecting deposits and generating genesis...$(NC)\n"
	@$(CHAIND_BIN) genesis collect-premined-deposits \
		--chaincfg.chain-spec=$(CHAIN_SPEC) --chaincfg.chain-spec-file=$(CONFIG_DIR)/spec.toml \
		--home $(DATA_DIR)/.tmp/0gchaind
	@$(CHAIND_BIN) genesis execution-payload $(CONFIG_DIR)/eth-genesis.json \
		--chaincfg.chain-spec=$(CHAIN_SPEC) --chaincfg.chain-spec-file=$(CONFIG_DIR)/spec.toml \
		--home $(DATA_DIR)/.tmp/0gchaind
	
	@# Save generated genesis for reference
	@cp $(DATA_DIR)/.tmp/0gchaind/config/genesis.json $(CONFIG_DIR)/cosmos-genesis.json
	
	@printf "$(GREEN)Deploy complete! Node data initialized.$(NC)\n"

#
# ============== START ==============
#

start: start-reth
	@sleep 3
	@$(MAKE) start-chain
	@printf "$(GREEN)Node started!$(NC)\n"
	@printf "$(YELLOW)RPC Endpoints:$(NC)\n"
	@printf "  Cosmos RPC:     http://localhost:$(CONSENSUS_RPC_PORT)\n"
	@printf "  ETH HTTP RPC:   http://localhost:$(ETH_HTTP_PORT)\n"
	@printf "  ETH WS RPC:     ws://localhost:$(ETH_WS_PORT)\n"

start-reth:
	@printf "$(GREEN)Starting reth...$(NC)\n"
	@mkdir -p $(DATA_DIR)/logs
	@chmod +x $(GETH_BIN)
	@nohup $(GETH_BIN) node \
		--chain $(CONFIG_DIR)/eth-genesis.json \
		--http --http.addr 0.0.0.0 --http.port $(ETH_HTTP_PORT) --http.api debug,eth,net,trace,txpool,web3,rpc --http.corsdomain "*" \
		--ws --ws.addr 0.0.0.0 --ws.port $(ETH_WS_PORT) --ws.api debug,eth,net,trace,txpool,web3,rpc \
		--port $(P2P_PORT) --discovery.port $(DISCOVERY_PORT) \
		--rpc.max-subscriptions-per-connection 20000 --rpc.max-connections 20000 \
		--txpool.pending-max-count 500000 --txpool.pending-max-size 1000 \
		--txpool.basefee-max-count 500000 --txpool.basefee-max-size 1000 \
		--txpool.queued-max-count 500000 --txpool.queued-max-size 1000 \
		--txpool.max-pending-txns 500000 \
		--txpool.max-account-slots 500000 \
		--txpool.max-new-pending-txs-notifications 500000 \
		--authrpc.addr 0.0.0.0 --authrpc.port $(AUTHRPC_PORT) --authrpc.jwtsecret $(CONFIG_DIR)/jwt.hex \
		--datadir $(DATA_DIR)/.tmp/eth-home \
		--ipcpath $(DATA_DIR)/.tmp/eth-home/eth-engine.ipc \
		--metrics 0.0.0.0:$(ETH_METRICS_PORT) \
		--engine.persistence-threshold 0 --engine.memory-block-buffer-target 0 \
		> $(DATA_DIR)/logs/reth.log 2>&1 < /dev/null & echo $$! > $(DATA_DIR)/reth.pid
	@printf "$(GREEN)reth started!$(NC)\n"

start-chain:
	@printf "$(GREEN)Starting 0gchaind...$(NC)\n"
	@mkdir -p $(DATA_DIR)/logs
	@chmod +x $(CHAIND_BIN)
	@nohup $(CHAIND_BIN) start \
		--home $(DATA_DIR)/.tmp/0gchaind \
		--pruning=nothing \
		--p2p.laddr="tcp://0.0.0.0:$(CONSENSUS_P2P_PORT)" \
		--rpc.laddr="tcp://0.0.0.0:$(CONSENSUS_RPC_PORT)" \
		--chaincfg.logger.log-level=debug \
		--chaincfg.engine.jwt-secret-path=$(CONFIG_DIR)/jwt.hex \
		--chaincfg.kzg.trusted-setup-path=$(CONFIG_DIR)/kzg-trusted-setup.json \
		--chaincfg.kzg.implementation="crate-crypto/go-kzg-4844" \
		--chaincfg.block-store-service.enabled \
		--chaincfg.node-api.enabled --chaincfg.node-api.address="0.0.0.0:$(NODE_API_PORT)" \
		--chaincfg.engine.rpc-dial-url="http://127.0.0.1:$(AUTHRPC_PORT)" \
		--chaincfg.chain-spec=$(CHAIN_SPEC) --chaincfg.chain-spec-file=$(CONFIG_DIR)/spec.toml \
		--chaincfg.restaking.enabled \
		--chaincfg.restaking.symbiotic-rpc-dial-url="http://127.0.0.1:$(ETH_HTTP_PORT)" \
		> $(DATA_DIR)/logs/chaind.log 2>&1 < /dev/null & echo $$! > $(DATA_DIR)/chaind.pid
	@printf "$(GREEN)0gchaind started!$(NC)\n"

#
# ============== STOP ==============
#

stop:
	@printf "$(YELLOW)Stopping all processes...$(NC)\n"
	-@if [ -f "$(DATA_DIR)/reth.pid" ]; then \
		kill $$(cat "$(DATA_DIR)/reth.pid") 2>/dev/null || true; \
		rm -f "$(DATA_DIR)/reth.pid"; \
	else \
		pkill -f "$(GETH_BIN)" 2>/dev/null || true; \
	fi
	-@if [ -f "$(DATA_DIR)/chaind.pid" ]; then \
		kill $$(cat "$(DATA_DIR)/chaind.pid") 2>/dev/null || true; \
		rm -f "$(DATA_DIR)/chaind.pid"; \
	else \
		pkill -f "$(CHAIND_BIN)" 2>/dev/null || true; \
	fi
	@sleep 2
	@printf "$(GREEN)All processes stopped!$(NC)\n"

restart: stop start

#
# ============== CLEAN ==============
#

clean:
	@printf "$(YELLOW)Cleaning data directory...$(NC)\n"
	@rm -rf $(DATA_DIR)
	@rm -f $(CONFIG_DIR)/cosmos-genesis.json
	@printf "$(GREEN)Data cleaned!$(NC)\n"

clean-all: clean
	@printf "$(YELLOW)Cleaning everything...$(NC)\n"
	@rm -rf $(BIN_DIR)
	@rm -rf $(SRC_DIR)
	@printf "$(GREEN)All cleaned!$(NC)\n"

#
# ============== STATUS & LOGS ==============
#

status:
	@printf "$(GREEN)=== Node Status ===$(NC)\n"
	@printf "$(YELLOW)Block Height:$(NC) "
	@curl -s http://localhost:$(CONSENSUS_RPC_PORT)/status 2>/dev/null | jq -r '.result.sync_info.latest_block_height // "Not running"' || echo "Not running"
	@printf "\n$(YELLOW)Processes:$(NC)\n"
	@pgrep -fa "geth|0gchaind" | head -5 || echo "No processes running"

logs:
	@tail -f $(DATA_DIR)/logs/chaind.log

logs-reth:
	@tail -f $(DATA_DIR)/logs/reth.log

#
# ============== ZIP ==============
#

zip:
	@printf "$(YELLOW)Zipping necessary files...$(NC)\n"
	@zip -r localtestnet1.zip Makefile .env README.md config
	@printf "$(GREEN)Zip complete: localtestnet1.zip$(NC)\n"
