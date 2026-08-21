use clap::{arg, command, Arg, Command};

pub fn cli_app() -> Command {
    command!()
        .arg(arg!(-c --config <FILE> "Sets a custom config file"))
        .arg(arg!(--"log-config-file" [FILE] "Sets log configuration file (Default: log_config)"))
        .arg(arg!(--"miner-key" [KEY] "Sets miner private key (Default: None)"))
        .arg(
            arg!(--"blockchain-rpc-endpoint" [URL] "Sets blockchain RPC endpoint (Default: http://127.0.0.1:8545)")
        )
        // `RawConfiguration::parse` looks each field up by its hyphenated field name, so
        // the clap id has to be `db-max-num-sectors` to be readable at all. The old
        // `--db-max-num-chunks` spelling stays as an alias: it is what our own Dockerfile
        // passes, and it has been accepted (though silently ignored) since #137.
        .arg(
            Arg::new("db-max-num-sectors")
                .long("db-max-num-sectors")
                .alias("db-max-num-chunks")
                .value_name("NUM")
                .num_args(0..=1)
                .help("Sets the max number of sectors to store in db (Default: None)"),
        )
        .arg(arg!(--"network-enr-address" [URL] "Sets the network ENR address (Default: None)"))
        .allow_external_subcommands(true)
        .version(zgs_version::VERSION)
}

#[cfg(test)]
mod tests {
    use super::cli_app;
    use crate::config::RawConfiguration;

    /// `RawConfiguration::parse` reads each CLI value by the hyphenated form of the field
    /// name, so an arg whose clap id is not a field name parses successfully and is then
    /// silently discarded. That is how `--db-max-num-chunks` went unread for 19 months:
    /// #137 renamed the field to `db_max_num_sectors` but left the flag alone. Release
    /// builds cannot catch it at runtime either - `ArgMatches::get_one` only panics on an
    /// unknown id under `debug_assertions`, which this workspace disables.
    #[test]
    fn every_cli_arg_maps_to_a_config_field() {
        let fields: Vec<String> = RawConfiguration::FIELD_NAMES
            .iter()
            .map(|name| name.replace('_', "-"))
            .collect();

        for arg in cli_app().get_arguments() {
            let id = arg.get_id().as_str();
            // `--config` names the config file itself and is read directly by `parse`.
            if matches!(id, "config" | "help" | "version") {
                continue;
            }

            assert!(
                fields.contains(&id.to_string()),
                "CLI arg `--{id}` does not match any RawConfiguration field, so its value \
                 would parse and then be silently ignored",
            );
        }
    }
}
