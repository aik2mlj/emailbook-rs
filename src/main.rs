mod emailbook;

use std::io::Write;
use std::path::PathBuf;
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use directories::BaseDirs;
use emailbook::EmailBook;

fn default_file() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.data_dir().join("emailbook.txt"))
        .unwrap_or_else(|| PathBuf::from("emailbook.txt"))
}

#[derive(Clone, Debug, ValueEnum)]
enum ParseSource {
    From,
    To,
    Cc,
    Bcc,
    All,
}

impl ParseSource {
    fn to_fields(&self) -> Vec<&'static str> {
        match self {
            Self::All => vec!["From:", "To:", "Cc:", "CC:", "Bcc:"],
            Self::From => vec!["From:"],
            Self::To => vec!["To:"],
            Self::Cc => vec!["Cc:", "CC:"],
            Self::Bcc => vec!["Bcc:"],
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Add an entry to the address book
    Add {
        /// Key/alias (if VALUE is also given), or e-mail address
        key_or_value: String,
        /// E-mail address (when KEY is provided)
        value: Option<String>,
    },
    /// Search the address book
    Search {
        /// Search only keys/aliases
        #[arg(short, long)]
        key: bool,
        /// Search only values/e-mail addresses
        #[arg(short, long)]
        value: bool,
        /// Query string
        query: String,
    },
    /// Parse stdin for e-mail addresses and add them to the address book
    Parse {
        /// Header source: from, to, cc, bcc, all (default: all)
        source: Option<ParseSource>,
    },
    /// Read filenames from stdin, open and parse them for e-mail addresses
    #[command(name = "parse-files")]
    ParseFiles {
        /// Header source: from, to, cc, bcc, all (default: all)
        source: Option<ParseSource>,
    },
    /// Remove entries from the address book interactively
    Remove {
        /// Search only keys/aliases
        #[arg(short, long)]
        key: bool,
        /// Search only values/e-mail addresses
        #[arg(short, long)]
        value: bool,
        /// Query string
        query: String,
    },
    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Parser)]
#[command(
    name = "emailbook",
    version,
    about = "A minimalistic address book for e-mails only",
    after_help = "If no subcommand is given, prints all entries."
)]
struct Cli {
    /// Address book file path [default: <data_dir>/emailbook.txt]
    #[arg(short, long)]
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

fn display_value(line: &str) -> &str {
    match line.find(':') {
        Some(pos) if pos + 2 <= line.len() => &line[pos + 2..],
        _ => line,
    }
}

fn main() {
    let cli = Cli::parse();

    if let Some(Commands::Completion { shell }) = cli.command {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "emailbook", &mut std::io::stdout());
        return;
    }

    let file = cli.file.unwrap_or_else(default_file);

    if let Some(parent) = file.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut book = match EmailBook::open(file.to_str().unwrap_or("emailbook.txt")) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error opening {}: {e}", file.display());
            process::exit(1);
        }
    };

    match cli.command {
        None => {
            for line in &book.lines {
                println!("{line}");
            }
        }
        Some(Commands::Add {
            key_or_value,
            value,
        }) => {
            if let Some(v) = value {
                book.add(Some(&key_or_value), &v);
            } else {
                book.add(None, &key_or_value);
            }
        }
        Some(Commands::Search { key, value, query }) => {
            let indices = if key {
                book.search_by_alias(&query)
            } else if value {
                book.search_by_value(&query)
            } else {
                book.search_all(&query)
            };
            for idx in indices {
                println!("{}", display_value(&book.lines[idx]));
            }
        }
        Some(Commands::Parse { source }) => {
            let fields = source.unwrap_or(ParseSource::All).to_fields();
            if let Err(e) = book.parse_stdin(&fields) {
                eprintln!("Error parsing stdin: {e}");
                process::exit(1);
            }
        }
        Some(Commands::ParseFiles { source }) => {
            let fields = source.unwrap_or(ParseSource::All).to_fields();
            if let Err(e) = book.parse_files(&fields) {
                eprintln!("Error parsing files: {e}");
                process::exit(1);
            }
        }
        Some(Commands::Remove { key, value, query }) => {
            let indices = if key {
                book.search_by_alias(&query)
            } else if value {
                book.search_by_value(&query)
            } else {
                book.search_all(&query)
            };

            let mut to_remove = Vec::new();
            for idx in indices {
                let line = &book.lines[idx];
                eprint!("{line}\n  Remove? [y/N] ");
                let _ = std::io::stderr().flush();
                let mut input = String::new();
                if std::io::stdin().read_line(&mut input).is_ok()
                    && input.trim().eq_ignore_ascii_case("y")
                {
                    println!("- {line}");
                    to_remove.push(idx);
                }
            }

            if !to_remove.is_empty()
                && let Err(e) = book.remove_lines(&to_remove)
            {
                eprintln!("Error writing file: {e}");
                process::exit(1);
            }
        }
        Some(Commands::Completion { .. }) => unreachable!(),
    }
}
