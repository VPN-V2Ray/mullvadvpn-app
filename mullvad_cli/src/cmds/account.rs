use Command;
use Result;
use clap;
use rpc;

pub struct Account;

impl Command for Account {
    fn name(&self) -> &'static str {
        "account"
    }

    fn clap_subcommand(&self) -> clap::App<'static, 'static> {
        clap::SubCommand::with_name(self.name())
            .about("Control and display information about your Mullvad account")
            .setting(clap::AppSettings::SubcommandRequired)
            .subcommand(clap::SubCommand::with_name("set")
                .about("Change account")
                .arg(clap::Arg::with_name("token")
                    .help("The Mullvad account token to configure the client with")
                    .required(true)))
            .subcommand(clap::SubCommand::with_name("get")
                .about("Display information about the currently configured account"))
    }

    fn run(&self, matches: &clap::ArgMatches) -> Result<()> {
        if let Some(set_matches) = matches.subcommand_matches("set") {
            let token = value_t_or_exit!(set_matches.value_of("token"), String);
            self.set(&token)
        } else if let Some(_matches) = matches.subcommand_matches("get") {
            self.get()
        } else {
            unreachable!("No account command given");
        }
    }
}

impl Account {
    fn set(&self, token: &str) -> Result<()> {
        rpc::call("set_account", &[token]).map(
            |_: Option<()>| {
                println!("Mullvad account \"{}\" set", token);
            },
        )
    }

    fn get(&self) -> Result<()> {
        let token: Option<String> = rpc::call("get_account", &[] as &[u8; 0])?;
        match token {
            Some(token) => println!("Mullvad account: {:?}", token),
            None => println!("No account configured"),
        }
        Ok(())
    }
}
