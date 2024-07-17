use clap::{Parser, ValueEnum};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Error, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Parser)]
#[command(arg_required_else_help(true))]
struct Cli {
    file_list: Vec<String>,
    #[arg(short, long, value_enum)]
    git: Option<GitFiles>,
    #[arg(short, long)]
    delimiter: Option<String>,
}

#[derive(ValueEnum, Debug, Clone)] // ArgEnum here
#[clap(rename_all = "kebab_case")]
enum GitFiles {
    All,
    Staged,
    Modified,
}

impl GitFiles {
    fn cli_args(&self) -> &'static [&'static str] {
        match self {
            GitFiles::All => &["ls-files"],
            GitFiles::Staged => &["diff", "--cached", "--name-only"],
            GitFiles::Modified => &["ls-files", "--modified"],
        }
    }

    fn get_file_list(&self) -> Result<Vec<String>, Error> {
        let output = std::process::Command::new("git")
            .args(self.cli_args())
            .output()?;

        output.stdout.lines().collect()
    }

    fn display(&self) -> &'static str {
        match self {
            GitFiles::All => "git files",
            GitFiles::Staged => "staged files",
            GitFiles::Modified => "modified files",
        }
    }
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    let file_list = match cli.git {
        Some(git_files) => {
            let files = git_files.get_file_list()?;
            let files = [files, cli.file_list].concat();

            if files.is_empty() {
                eprintln!("there are no {} to sort", git_files.display());
                return Ok(());
            }

            files
        }
        None => cli.file_list,
    };

    for file in &file_list {
        match sort_lines(&cli.delimiter, file) {
            Ok(had_changes) => {
                if had_changes {
                    println!("{}", file);
                } else {
                    //prints in grey text
                    println!("\x1b[30m{}\x1b[0m", file);
                }
            }
            Err(e) => {
                println!("error sorting {}: {}", file, e);
            }
        }
    }

    Ok(())
}

fn sort_lines(delimiter: &Option<String>, file: &str) -> Result<bool, Error> {
    let file_path = Path::new(file);

    let delim = delimiter
        .as_deref()
        .or_else(|| {
            file_path
                .extension()
                .and_then(|str| str.to_str())
                .map(auto_detect_delim)
        })
        .unwrap_or("//");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .append(false)
        .open(file_path)?;

    let mut reader = BufReader::new(&file);

    let mut cur_string = String::new();
    let mut line = reader.read_line(&mut cur_string)?;

    // This is start position of comment block, end position, then the list of strings
    let mut words_list: Vec<(u64, u64, Vec<String>)> = Vec::new();
    words_list.push((0, 0, Vec::new()));

    let mut num_sort_blocks: usize = 0;

    loop {
        if line == 0 {
            break;
        }

        if set_lines(
            delim,
            &mut cur_string,
            &mut reader,
            &mut words_list[num_sort_blocks],
        )? {
            num_sort_blocks += 1;
            words_list.push((0, 0, Vec::new()));
        }
        cur_string.clear();
        line = reader.read_line(&mut cur_string)?;
    }

    // End pos was never set, so we never hit the end of the sort-lines block

    let mut writer = BufWriter::new(&file);

    for word_tuple in words_list.iter() {
        //No end, so dont change anything
        if word_tuple.1 == 0 {
            continue;
        }
        let _ = writer.seek(SeekFrom::Start(word_tuple.0));

        // println!("{}", words.join(""));

        let _ = writer.write_all(word_tuple.2.join("").as_bytes());

        writer.flush()?;
    }

    Ok(num_sort_blocks > 0)
}
fn insertion_sort(list: &mut Vec<String>, insert_string: String) {
    for i in 0..list.len() {
        if insert_string.to_lowercase() < list[i].to_lowercase() {
            list.insert(i, insert_string);
            return;
        }
    }
    list.push(insert_string);
}

fn set_lines(
    delim: &str,
    current_string: &mut String,
    reader: &mut BufReader<&File>,
    data: &mut (u64, u64, Vec<String>),
) -> Result<bool, Error> {
    if current_string.trim_start() == delim.to_owned() + " sort-lines: start\n" {
        data.0 = reader.stream_position()?;

        // Clear the first comment line
        current_string.clear();
        let mut line = reader.read_line(current_string)?;

        // Iterate until we hit the end of the sort-lines block
        while current_string.trim_start() != delim.to_owned() + " sort-lines: end\n" {
            // Unwind to the beginning of the sort-line block, as we have reached EOF without an end
            if line == 0 {
                let _ = reader.seek(SeekFrom::Start(data.0 + 1));
                return Ok(true);
            }
            insertion_sort(&mut data.2, current_string.clone());

            current_string.clear();
            line = reader.read_line(current_string)?;
        }
        data.1 = reader.stream_position().unwrap();
        return Ok(true);
    }

    Ok(false)
}

fn auto_detect_delim(extension: &str) -> &str {
    match extension.to_lowercase().as_str() {
        "sh" | "bash" | "fish" | "nu" => "#",
        "py" | "rb" | "pl" | "ex" | "nix" | "toml" | "yaml" => "#",
        "lua" | "hs" | "lhs" | "sql" => "--",
        "ini" | "asm" | "s" => ";",
        "bat" | "cmd" => "@REM",
        "c" | "c++" | "cpp" => "//",
        "js" | "ts" => "//",

        // Most languages use `//` for comments
        _ => "//",
    }
}
