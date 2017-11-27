use std::env;

use Flags;
use tally::Row;

pub(crate) fn print_csv(flags: &Flags, table: &[Row]) {
    print!("timestamp");
    for s in &flags.arg_crate {
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
            if flags.flag_relative {
                print!(",{}", column as f32 / row.total as f32);
            } else {
                print!(",{}", column);
            }
        }
        println!();
    }
}
