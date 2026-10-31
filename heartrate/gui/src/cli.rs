#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    pub no_gui: bool,
    pub register: bool,
    pub unregister: bool,
    pub autostart: Option<bool>,
    pub verbose: bool,
    pub help: bool,
    pub version: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgError(pub String);

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub const USAGE: &str = "\
Heartrate OSC - sends your heart rate to VRChat, and shows it in the SteamVR dashboard.

Usage: heartrate_gui [OPTIONS]

Options:
      --no-gui             Run without the desktop window (heart rate output and
                           the SteamVR dashboard tab keep working)
      --register           Register with SteamVR so it can list and start this app
      --unregister         Remove the SteamVR registration and auto-start
      --autostart <BOOL>   Start this app whenever SteamVR starts (true or false)
      --verbose            Include debug-level messages in the log
  -h, --help               Print this help
  -V, --version            Print the version

The one-shot commands (--register, --unregister, --autostart) need SteamVR to be
running, and exit once they are done.";

impl Cli {
    pub fn parse<I, S>(args: I) -> Result<Self, ArgError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut cli = Cli::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_ref() {
                "--no-gui" => cli.no_gui = true,
                "--register" => cli.register = true,
                "--unregister" => cli.unregister = true,
                "--verbose" => cli.verbose = true,
                "-h" | "--help" => cli.help = true,
                "-V" | "--version" => cli.version = true,
                "--autostart" => {
                    let value = args
                        .next()
                        .ok_or_else(|| ArgError("--autostart needs true or false".into()))?;
                    cli.autostart = Some(parse_bool(value.as_ref())?);
                }
                other if other.starts_with("--autostart=") => {
                    cli.autostart = Some(parse_bool(&other["--autostart=".len()..])?);
                }
                other => {
                    return Err(ArgError(format!(
                        "unrecognised argument `{other}`. Try --help."
                    )))
                }
            }
        }

        if cli.register && cli.unregister {
            return Err(ArgError(
                "--register and --unregister contradict each other".into(),
            ));
        }

        Ok(cli)
    }

    pub fn from_env() -> Result<Self, ArgError> {
        Self::parse(std::env::args().skip(1))
    }

    pub fn is_oneshot(&self) -> bool {
        self.register || self.unregister || self.autostart.is_some()
    }
}

fn parse_bool(value: &str) -> Result<bool, ArgError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        other => Err(ArgError(format!("`{other}` is not true or false"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_starts_the_application_with_a_window() {
        let cli = Cli::parse(Vec::<String>::new()).unwrap();
        assert_eq!(cli, Cli::default());
        assert!(!cli.no_gui);
        assert!(!cli.is_oneshot());
    }

    #[test]
    fn the_manifest_passes_no_gui() {
        let cli = Cli::parse(["--no-gui"]).unwrap();
        assert!(cli.no_gui);
        assert!(!cli.is_oneshot(), "it must start the app, not exit");
    }

    #[test]
    fn autostart_takes_its_value_either_way_round() {
        assert_eq!(Cli::parse(["--autostart", "true"]).unwrap().autostart, Some(true));
        assert_eq!(Cli::parse(["--autostart=false"]).unwrap().autostart, Some(false));
        assert_eq!(Cli::parse(["--autostart", "ON"]).unwrap().autostart, Some(true));
        assert_eq!(Cli::parse(["--autostart=0"]).unwrap().autostart, Some(false));
        assert!(Cli::parse(["--autostart"]).is_err());
        assert!(Cli::parse(["--autostart", "maybe"]).is_err());
    }

    #[test]
    fn one_shot_commands_are_recognised_as_such() {
        assert!(Cli::parse(["--register"]).unwrap().is_oneshot());
        assert!(Cli::parse(["--unregister"]).unwrap().is_oneshot());
        assert!(Cli::parse(["--autostart", "false"]).unwrap().is_oneshot());
        assert!(!Cli::parse(["--verbose"]).unwrap().is_oneshot());
    }

    #[test]
    fn contradictory_and_unknown_arguments_are_refused() {
        assert!(Cli::parse(["--register", "--unregister"]).is_err());
        assert!(Cli::parse(["--nope"]).is_err());
        assert!(Cli::parse(["--nogui"]).is_err());
    }

    #[test]
    fn flags_combine() {
        let cli = Cli::parse(["--no-gui", "--verbose"]).unwrap();
        assert!(cli.no_gui && cli.verbose);
    }
}
