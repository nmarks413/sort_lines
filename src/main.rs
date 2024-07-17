use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[clap(required = true)]
    file_list: Vec<String>,
    #[arg(short, long)]
    delimiter: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    println!("{:?}", cli.file_list);

    for file in cli.file_list {
        let file_path = Path::new(&file);

        let delim = cli
            .delimiter
            .as_deref()
            .or_else(|| {
                file_path
                    .extension()
                    .and_then(|str| str.to_str())
                    .map(|str| auto_detect_delim(str))
            })
            .unwrap_or("//");

        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .open(file_path)
        {
            Ok(f) => f,
            Err(_) => {
                println!("Could not find {}", file);
                continue;
            }
        };

        let mut reader = BufReader::new(&file);

        let mut cur_string = String::new();
        let mut line = reader.read_line(&mut cur_string).unwrap();

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
            ) {
                num_sort_blocks += 1;
                words_list.push((0, 0, Vec::new()));
            }
            cur_string.clear();
            line = reader.read_line(&mut cur_string).unwrap();
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

            writer.flush().unwrap();
        }
    }
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
) -> bool {
    if current_string.trim_start() == delim.to_owned() + " sort-lines: start\n" {
        data.0 = reader.stream_position().unwrap();

        // Clear the first comment line
        current_string.clear();
        let mut line = reader.read_line(current_string).unwrap();

        // Iterate until we hit the end of the sort-lines block
        while current_string.trim_start() != delim.to_owned() + " sort-lines: end\n" {
            // Unwind to the beginning of the sort-line block, as we have reached EOF without an end
            if line == 0 {
                let _ = reader.seek(SeekFrom::Start(data.0 + 1));
                return true;
            }
            insertion_sort(&mut data.2, current_string.clone());

            current_string.clear();
            line = reader.read_line(current_string).unwrap();
        }
        data.1 = reader.stream_position().unwrap();
        return true;
    }
    false
}

fn auto_detect_delim(extension: &str) -> &str {
    match extension.to_lowercase().as_str() {
        "sh" | "bash" | "fish" | "py" | "rb" | "pl" | "ex" => "#",
        "lua" | "hs" | "lhs" | "sql" => "--",
        "ini" | "asm" => ";",
        "bat" | "cmd" => "@REM",

        // Most languages use `//` for comments
        _ => "//",
    }
}
