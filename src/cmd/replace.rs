use regex::bytes::RegexBuilder;
use std::borrow::Cow;
use std::env;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;
use serde::Deserialize;

static USAGE: &str = "
Replace occurrences of a pattern across a CSV file.

You can of course match groups using parentheses and use those in
the replacement string. But don't forget to escape your $ in bash by using a
backslash or by wrapping the replacement string into single quotes:

  $ qsv replace 'hel(lo)' 'hal$1' file.csv
  $ qsv replace \"hel(lo)\" \"hal\\$1\" file.csv

Usage:
    qsv replace [options] <pattern> <replacement> [<input>]
    qsv replace --help

replace options:
    -i, --ignore-case      Case insensitive search. This is equivalent to
                           prefixing the regex with '(?i)'.
    -s, --select <arg>     Select the columns to search. See 'qsv select -h'
                           for the full syntax.
    -u, --unicode          Enable unicode support. When enabled, character classes
                           will match all unicode word characters instead of only
                           ASCII word characters. Decreases performance.                            

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. (default: ,)
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_pattern: String,
    arg_replacement: String,
    flag_select: SelectColumns,
    flag_unicode: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_ignore_case: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let regex_unicode = match env::var("QSV_REGEX_UNICODE") {
        Ok(_) => true,
        Err(_) => args.flag_unicode,
    };
    let pattern = RegexBuilder::new(&*args.arg_pattern)
        .case_insensitive(args.flag_ignore_case)
        .unicode(regex_unicode)
        .build()?;
    let replacement = args.arg_replacement.as_bytes();
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    // NOTE: using vec lookups is not the fastest thing in the world but
    // I am not sure it would be worthwhile to rely on a set structure
    let sel_indices = sel.to_vec();

    if !rconfig.no_headers {
        wtr.write_record(&headers)?;
    }
    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        record = record
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                if sel_indices.contains(&i) {
                    pattern.replace_all(v, replacement)
                } else {
                    Cow::Borrowed(v)
                }
            })
            .collect();

        wtr.write_byte_record(&record)?;
    }
    Ok(wtr.flush()?)
}
