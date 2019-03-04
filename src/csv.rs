use std::env;

use crate::tally::Row;
use crate::Args;

pub(crate) fn print_csv(args: &Args, table: &[Row]) {
    print!("timestamp");
    for s in &args.crates {
        print!(",{}", s);
    }
    println!();

    let detail = env::var("DETAIL").is_ok();

    for row in table {
        print!("{}", row.timestamp.format("%m/%d/%Y %H:%M"));
        if detail {
            print!(",{}:{}", row.name, row.num);
        }
        for &column in &row.counts {
            if args.relative {
                print!(",{}", column as f32 / row.total as f32);
            } else {
                print!(",{}", column);
            }
        }
        println!();
    }
}
