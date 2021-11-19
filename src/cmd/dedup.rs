use std::cmp;
use std::str::from_utf8;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;
use serde::Deserialize;

use crate::cmd::sort::iter_cmp;

static USAGE: &str = "
Dedups CSV rows. 

Note that this requires reading all of the CSV data into memory, because the rows need to be sorted first.

Usage:
    qsv dedup [options] [<input>]

sort options:
    -s, --select <arg>         Select a subset of columns to dedup.
                               Note that the outputs will remain at the full width
                               of the CSV.
                               See 'qsv select --help' for the format details.
    -C, --no-case              Compare strings disregarding case
    -D, --dupes-output <file>  Write duplicates to <file>.

Common options:
    -h, --help                 Display this message
    -o, --output <file>        Write output to <file> instead of stdout.
    -n, --no-headers           When set, the first row will not be interpreted
                               as headers. Namely, it will be sorted with the rest
                               of the rows. Otherwise, the first row will always
                               appear as the header row in the output.
    -d, --delimiter <arg>      The field delimiter for reading CSV data.
                               Must be a single character. (default: ,)
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_no_case: bool,
    flag_dupes_output: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    log::info!("cmd: dedup, input: {:?}, select: {:?}, no_case: {}, dupes_output: {:?}, output: {:?}, no_header: {}, delimiter: {:?}", 
        (&args.arg_input).clone().unwrap(),
        &args.flag_select,
        &args.flag_no_case,
        (&args.flag_dupes_output).clone().unwrap(),
        (&args.flag_output).clone().unwrap(),
        &args.flag_no_headers,
        &args.flag_delimiter
    );

    let no_case = args.flag_no_case;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);



    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let dupes_output = args.flag_dupes_output.is_some();
    let mut dupewtr = Config::new(&args.flag_dupes_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    if dupes_output {
        dupewtr.write_byte_record(&headers)?;
    }
    let sel = rconfig.selection(&headers)?;

    let mut new: Vec<_> = vec![];
    {
        let mut all = rdr.byte_records().collect::<Result<Vec<_>, _>>()?;
        all.sort_by(|r1, r2| {
            let a = sel.select(r1);
            let b = sel.select(r2);
            iter_cmp(a, b)
        });

        let mut current = 0;
        while current + 1 < all.len() {
            let a = sel.select(&all[current]);
            let b = sel.select(&all[current + 1]);
            if no_case {
                if iter_cmp_no_case(a, b) != cmp::Ordering::Equal {
                    new.push(all[current].clone());
                } else if dupes_output {
                    dupewtr.write_byte_record(&all[current])?;
                }
            } else if iter_cmp(a, b) != cmp::Ordering::Equal {
                new.push(all[current].clone());
            } else if dupes_output {
                dupewtr.write_byte_record(&all[current])?;
            }
            current += 1;
        }
        new.push(all[current].clone());
    }

    dupewtr.flush()?;
    rconfig.write_headers(&mut rdr, &mut wtr)?;
    for r in new.into_iter() {
        wtr.write_byte_record(&r)?;
    }
    Ok(wtr.flush()?)
}

/// Try comparing `a` and `b` ignoring the case
pub fn iter_cmp_no_case<'a, L, R>(mut a: L, mut b: R) -> cmp::Ordering
where
    L: Iterator<Item = &'a [u8]>,
    R: Iterator<Item = &'a [u8]>,
{
    loop {
        match (next_no_case(&mut a), next_no_case(&mut b)) {
            (None, None) => return cmp::Ordering::Equal,
            (None, _) => return cmp::Ordering::Less,
            (_, None) => return cmp::Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                cmp::Ordering::Equal => (),
                non_eq => return non_eq,
            },
        }
    }
}

fn next_no_case<'a, X>(xs: &mut X) -> Option<String>
where
    X: Iterator<Item = &'a [u8]>,
{
    xs.next()
        .and_then(|bytes| from_utf8(bytes).ok())
        .map(|s| s.to_lowercase())
}
