mod emailbook;

use std::process;

use clap::Parser;
use emailbook::EmailBook;

#[derive(Parser)]
#[command(
    name = "emailbook",
    version,
    about = "A minimalistic address book for e-mails only",
    after_help = "If no option is given, prints all values in FILE.\n\
                  If only QUERY is given (without flags), searches keys and values."
)]
struct Cli {
    /// Address book file path
    file: String,

    /// Add entry to FILE. Use 'KEY VALUE' for keyed entry or just 'VALUE'
    #[arg(short, long, num_args = 1..=2, value_names = &["KEY_OR_VALUE", "VALUE"])]
    add: Option<Vec<String>>,

    /// Search keys and values for QUERY
    #[arg(short, long)]
    search: Option<String>,

    /// Search only keys/aliases for QUERY
    #[arg(short, long)]
    key: Option<String>,

    /// Search only values for QUERY
    #[arg(short, long)]
    value: Option<String>,

    /// Parse stdin for e-mail addresses in headers and add them to FILE.
    /// SOURCE: --from, --to, --cc, --bcc, or --all (default)
    #[arg(short, long, allow_hyphen_values = true)]
    parse: Option<Option<ParseSource>>,

    /// Read filenames from stdin, open and parse them for e-mail addresses.
    /// SOURCE: --from, --to, --cc, --bcc, or --all (default)
    #[arg(long, allow_hyphen_values = true)]
    parse_files: Option<Option<ParseSource>>,

    /// Bare query (search without -s flag)
    #[arg(hide = true)]
    query: Option<String>,
}

#[derive(Clone, Debug)]
enum ParseSource {
    From,
    To,
    Cc,
    Bcc,
    All,
}

impl std::str::FromStr for ParseSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.strip_prefix("--").unwrap_or(s) {
            "from" => Ok(Self::From),
            "to" => Ok(Self::To),
            "cc" => Ok(Self::Cc),
            "bcc" => Ok(Self::Bcc),
            "all" => Ok(Self::All),
            other => Err(format!("unknown source '{other}', expected: from, to, cc, bcc, all")),
        }
    }
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

fn main() {
    let cli = Cli::parse();

    let mut book = match EmailBook::open(&cli.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error opening {}: {e}", cli.file);
            process::exit(1);
        }
    };

    if let Some(args) = cli.add {
        if args.len() == 2 {
            book.add(Some(&args[0]), &args[1]);
        } else {
            book.add(None, &args[0]);
        }
    } else if let Some(query) = cli.search {
        book.search_all(&query);
    } else if let Some(query) = cli.key {
        book.search_by_alias(&query);
    } else if let Some(query) = cli.value {
        book.search_by_value(&query);
    } else if let Some(source) = cli.parse {
        let fields = source.unwrap_or(ParseSource::All).to_fields();
        if let Err(e) = book.parse_stdin(&fields) {
            eprintln!("Error parsing stdin: {e}");
            process::exit(1);
        }
    } else if let Some(source) = cli.parse_files {
        let fields = source.unwrap_or(ParseSource::All).to_fields();
        if let Err(e) = book.parse_files(&fields) {
            eprintln!("Error parsing files: {e}");
            process::exit(1);
        }
    } else if let Some(query) = cli.query {
        book.search_all(&query);
    } else {
        // No option: print all entries
        for line in &book.lines {
            println!("{line}");
        }
    }
}
