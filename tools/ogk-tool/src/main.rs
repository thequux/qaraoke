extern crate ogk;
extern crate clap;
use clap::{Arg,App,SubCommand};
use std::fs;
use std::io::BufReader;

fn main() {
    let matches = App::new("OGK tool")
        .version("0.1")
        .author("TQ Hirsch <thequux@thequux.com>")
        .subcommand(SubCommand::with_name("mux")
                    .about("Mux multiple streams into one file")
                    .arg(Arg::with_name("OUTPUT")
                         .required(true))
                    .arg(Arg::with_name("cdg")
                         .long("cdg")
                         .multiple(true)
                         .number_of_values(1)
                         .value_name("FILE"))
                    .arg(Arg::with_name("mp3")
                         .long("mp3")
                         .multiple(true)
                         .number_of_values(1)
                         .value_name("FILE")))
        .get_matches();
    match matches.subcommand() {
        ("mux", Some(matches)) => {
            let mut mux = ogk::ogg::OgkMux::new();
            if let Some(values) = matches.values_of_os("mp3") {
                for file in values {
                    use ogk::mp3::OggMP3Coder;
                    match fs::File::open(file).map(BufReader::new).and_then(OggMP3Coder::new).map(Box::new) {
                        Err(e) => {
                            println!("Failed to open MP3 file {:?}: {}", file, e);
                            std::process::exit(1);
                        },
                        Ok(f) => mux.add_stream(f),
                    }
                }
            }
            if let Some(values) = matches.values_of_os("cdg") {
                use ogk::cdg::OggCdgCoder;
                for file in values {
                    match fs::File::open(file).map(BufReader::new).map(OggCdgCoder::new).map(Box::new) {
                        Err(e) => {
                            println!("Failed to open CDG file {:?}: {}", file, e);
                            std::process::exit(1);
                        },
                        Ok(f) => mux.add_stream(f),
                    }
                }
            }

            let ofile = fs::File::create(matches.value_of_os("OUTPUT").unwrap()).expect("Failed to open output file");
            mux.write_to(ofile).expect("Failed to write output file");
        },
        (_, _) => println!("{}", matches.usage()),
    }
}

//fn open_stream(filename: P, xform: 
//    <P: AsRef<Path>,B: ogk::ogg::BitstreamCoder,F: FnOnce(fs::File) -> B> (filename:)
        
