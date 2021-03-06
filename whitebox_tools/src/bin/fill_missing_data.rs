extern crate whitebox_tools;
extern crate time;

use std::io;
use std::env;
use std::path;
use std::f64;
use whitebox_tools::raster::*;
use whitebox_tools::structures::fixed_radius_search::FixedRadiusSearch;

fn main() {
    let sep: String = path::MAIN_SEPARATOR.to_string();
    let mut input_file = String::new();
    let mut output_file = String::new();
    let mut working_directory: String = "".to_string();
    let mut filter_size = 11usize;
    let mut verbose: bool = false;
    let mut keyval: bool;
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 { panic!("Tool run with no paramters. Please see help (-h) for parameter descriptions."); }
    for i in 0..args.len() {
        let mut arg = args[i].replace("\"", "");
        arg = arg.replace("\'", "");
        let cmd = arg.split("="); // in case an equals sign was used
        let vec = cmd.collect::<Vec<&str>>();
        keyval = false;
        if vec.len() > 1 { keyval = true; }
        if vec[0].to_lowercase() == "-i" || vec[0].to_lowercase() == "--input" {
            if keyval {
                input_file = vec[1].to_string();
            } else {
                input_file = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
            if keyval {
                output_file = vec[1].to_string();
            } else {
                output_file = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-wd" || vec[0].to_lowercase() == "--wd" {
            if keyval {
                working_directory = vec[1].to_string();
            } else {
                working_directory = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-filter" || vec[0].to_lowercase() == "--filter" {
            if keyval {
                filter_size = vec[1].to_string().parse::<usize>().unwrap();
            } else {
                filter_size = args[i+1].to_string().parse::<usize>().unwrap();
            }
        } else if vec[0].to_lowercase() == "-v" || vec[0].to_lowercase() == "--verbose" {
            verbose = true;
        } else if vec[0].to_lowercase() == "-h" || vec[0].to_lowercase() == "--help" ||
            vec[0].to_lowercase() == "--h"{
            let mut s: String = "Help:\n".to_owned();
                     s.push_str("-i       Input LAS file (classification).\n");
                     s.push_str("-o       Output HTML file.\n");
                     s.push_str("-wd      Optional working directory. If specified, filenames parameters need not include a full path.\n");
                     s.push_str("-filter  Size of the filter kernel (default is 11).\n");
                     s.push_str("-version Prints the tool version number.\n");
                     s.push_str("-h       Prints help information.\n\n");
                     s.push_str("Example usage:\n\n");
                     s.push_str(&">> .*fill_missing_data -wd *path*to*data* -i input.dep -o NoOTOs.dep -filter 25 -slope 15.0\n".replace("*", &sep));
            println!("{}", s);
            return;
        } else if vec[0].to_lowercase() == "-version" || vec[0].to_lowercase() == "--version" {
            const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
            println!("slope v{}", VERSION.unwrap_or("unknown"));
            return;
        }
    }

    match run(input_file, output_file, working_directory, filter_size, verbose) {
        Ok(()) => println!("Complete!"),
        Err(err) => panic!("{}", err),
    }
}

fn run(mut input_file: String, mut output_file: String, mut working_directory: String,
    mut filter_size: usize, verbose: bool) -> Result<(), io::Error> {

    if verbose {
        println!("********************************");
        println!("* Welcome to fill_missing_data *");
        println!("********************************");
    }

    let sep: String = path::MAIN_SEPARATOR.to_string();

	// The filter dimensions must be odd numbers such that there is a middle pixel
    if (filter_size as f64 / 2f64).floor() == (filter_size as f64 / 2f64) {
        filter_size += 1;
    }

    let mut z: f64;
    let (mut row_n, mut col_n): (isize, isize);
    let mut progress: usize;
    let mut old_progress: usize = 1;

    if !working_directory.ends_with(&sep) {
        working_directory.push_str(&(sep.to_string()));
    }

    if !input_file.contains(&sep) {
        input_file = format!("{}{}", working_directory, input_file);
    }
    if !output_file.contains(&sep) {
        output_file = format!("{}{}", working_directory, output_file);
    }

    if verbose { println!("Reading data...") };

    let input = Raster::new(&input_file, "r")?;
    let mut output = Raster::initialize_using_file(&output_file, &input);

    let start = time::now();

    let nodata = input.configs.nodata;
    let columns = input.configs.columns as isize;
    let rows = input.configs.rows as isize;
    let d_x = [ 1, 1, 1, 0, -1, -1, -1, 0 ];
	let d_y = [ -1, 0, 1, 1, 1, 0, -1, -1 ];

    // Interpolate the data holes. Start by locating all the edge cells.
    if verbose { println!("Interpolating data holes...") };
    let mut frs: FixedRadiusSearch<f64> = FixedRadiusSearch::new(filter_size as f64);
    for row in 0..rows {
        for col in 0..columns {
            if input[(row, col)] != nodata {
                for i in 0..8 {
                    row_n = row + d_y[i];
                    col_n = col + d_x[i];
                    if input[(row_n, col_n)] == nodata {
                        frs.insert(col as f64, row as f64, input[(row, col)]);
                        break;
                    }
                }
            }
        }
        if verbose {
            progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
            if progress != old_progress {
                println!("Finding OTO edge cells: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let mut sum_weights: f64;
    let mut dist: f64;
    for row in 0..rows {
        for col in 0..columns {
            if input[(row, col)] == nodata {
                sum_weights = 0f64;
                let ret = frs.search(col as f64, row as f64);
                for j in 0..ret.len() {
                    dist = ret[j].1;
                    if dist > 0.0 {
                        sum_weights += 1.0 / (dist * dist);
                    }
                }
                z = 0.0;
                for j in 0..ret.len() {
                    dist = ret[j].1;
                    if dist > 0.0 {
                        z += ret[j].0 * (1.0 / (dist * dist)) / sum_weights;
                    }
                }
                output[(row, col)] = z;
            } else {
                output[(row, col)] = input[(row, col)];
            }
        }
        if verbose {
            progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
            if progress != old_progress {
                println!("Interpolating data holes: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let end = time::now();
    let elapsed_time = end - start;

    output.add_metadata_entry("Created by whitebox_tools\' fill_missing_data tool".to_owned());
    output.add_metadata_entry(format!("Filter size: {}", filter_size));
    output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

    if verbose { println!("Saving data...") };
    let _ = match output.write() {
        Ok(_) => if verbose { println!("Output file written") },
        Err(e) => return Err(e),
    };





    ///////////////////////////////////////////////////////////////////////////////////////////////
    // NOTE:
    // The following disused code is for calculating a tophat transform with a circular shaped
    // structuring element (SE). It's no longer used because the square SE can be used in a way
    // that saves intermediate values and improves performance very considerably.
    ///////////////////////////////////////////////////////////////////////////////////////////////
    //fill the filter kernel cell offset values
    // let num_pixels_in_filter = filter_size * filter_size;
    // let mut d_x = vec![0isize; num_pixels_in_filter];
    // let mut d_y = vec![0isize; num_pixels_in_filter];
    // let mut filter_shape = vec![false; num_pixels_in_filter];
    //
    //see which pixels in the filter lie within the largest ellipse
    //that fits in the filter box
    // let sq = midpoint * midpoint;
    // let mut a = 0usize;
    // for row in 0..filter_size {
    //     for col in 0..filter_size {
    //         d_x[a] = col as isize - midpoint as isize;
    //         d_y[a] = row as isize - midpoint as isize;
    //         z = (d_x[a] * d_x[a]) as f64 / sq as f64 + (d_y[a] * d_y[a]) as f64 / sq as f64;
    //         if z <= 1f64 {
    //             filter_shape[a] = true;
    //         }
    //         a += 1;
    //     }
    // }
    // for row in 0..rows {
    //     for col in 0..columns {
    //         z = input.get_value(row, col);
    //         if z != nodata {
    //             let mut min_val = f64::INFINITY;
    //             for i in 0..num_pixels_in_filter {
    //                 z_n = input.get_value(row + d_y[i], col + d_x[i]);
    //                 if z_n < min_val && filter_shape[i] && z_n != nodata { min_val = z_n }
    //             }
    //             erosion[(row, col)] = min_val;
    //         } else {
    //             erosion[(row, col)] = nodata;
    //             opening[(row, col)] = nodata;
    //             tophat[(row, col)] = nodata;
    //         }
    //     }
    //     if verbose {
    //         progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
    //         if progress != old_progress {
    //             println!("Performing Erosion: {}%", progress);
    //             old_progress = progress;
    //         }
    //     }
    // }
    //
    // let (mut row_n, mut col_n): (isize, isize);
    // for row in 0..rows {
    //     for col in 0..columns {
    //         z = input.get_value(row, col);
    //         if z != nodata {
    //             let mut max_val = f64::NEG_INFINITY;
    //             for i in 0..num_pixels_in_filter {
    //                 col_n = col + d_x[i];
    //                 row_n = row + d_y[i];
    //                 z_n = erosion[(row_n, col_n)];
    //                 if z_n > max_val && filter_shape[i] && z_n != nodata { max_val = z_n }
    //             }
    //             tophat[(row, col)] = z - max_val;
    //             opening[(row, col)] = max_val;
    //         }
    //     }
    //     if verbose {
    //         progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
    //         if progress != old_progress {
    //             println!("Performing Dilation: {}%", progress);
    //             old_progress = progress;
    //         }
    //     }
    // }



    ///////////////////////////////////////////////////////////////////////////////////////////////
    // NOTE:
    // This disused code perfomed peak cleaving using a modified depression filling algorithm on
    // the tophat transform. The current method of region growing is more straight forward.
    ///////////////////////////////////////////////////////////////////////////////////////////////
    // find grid cells with nodata neighbours
    // let multiplier = 10000f64;
    // let mut heap = BinaryHeap::new();
    // let initial_value = f64::NEG_INFINITY;
    // let mut num_solved_cells = 0usize;
    // let num_cells = rows * columns;
    // let d_x = [ 1, 1, 1, 0, -1, -1, -1, 0 ];
	// let d_y = [ -1, 0, 1, 1, 1, 0, -1, -1 ];
    // for row in 0..rows as isize {
    //     for col in 0..columns as isize {
    //         output.set_value(row, col, initial_value);
    //         z = input.get_value(row, col);
    //         if z != nodata {
    //             let mut flag = false;
    //             for i in 0..8 {
    //                 z_n = input.get_value(row + d_y[i], col + d_x[i]);
    //                 if z_n == nodata {
    //                     flag = true;
    //                 }
    //             }
    //             if flag {
    //                 heap.push(GridCell { priority: -(tophat[row as usize][col as usize] * multiplier).floor() as isize, row: row, column: col });
    //                 output.set_value(row, col, tophat[row as usize][col as usize]);
    //                 num_solved_cells += 1;
    //             }
    //         } else {
    //             output.set_value(row, col, nodata);
    //         }
    //     }
    //     if verbose {
    //         progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
    //         if progress != old_progress {
    //             println!("Progress: {}%", progress);
    //             old_progress = progress;
    //         }
    //     }
    // }
    //
    // let (mut row, mut col): (isize, isize);
    // let mut frs: FixedRadiusSearch<f64> = FixedRadiusSearch::new(filter_size as f64);
    // let mut modified = vec![vec![false; columns]; rows];
    // while heap.len() > 0 {
    //     let gc = heap.pop().unwrap();
    //     row = gc.row;
    //     col = gc.column;
    //     z = -(gc.priority as f64 / multiplier);
    //     for i in 0..8 {
    //         row_n = row + d_y[i];
    //         col_n = col + d_x[i];
    //         if col_n >= 0 && col_n < columns as isize && row_n >= 0 && row_n < rows as isize {
    //             z_n = tophat[row_n as usize][col_n as usize];
    //             if z_n != nodata && output.get_value(row_n, col_n) == initial_value {
    //                 if z_n - z >= height_diff_threshold { //z_n >= z {
    //                     z_n = z;
    //                     modified[row_n as usize][col_n as usize] = true;
    //                     if !modified[row as usize][col as usize] {
    //                         frs.insert(col as f64, row as f64, tophat[row as usize][col as usize]);
    //                     }
    //                 }
    //                 output.set_value(row_n, col_n, z_n);
    //                 num_solved_cells += 1;
    //                 heap.push(GridCell { priority: -(z_n * multiplier).floor() as isize, row: row_n, column: col_n });
    //             }
    //         }
    //     }
    //     if verbose {
    //         progress = (100.0_f64 * num_solved_cells as f64 / (num_cells - 1) as f64) as usize;
    //         if progress != old_progress {
    //             println!("Progress: {}%", progress);
    //             old_progress = progress;
    //         }
    //     }
    // }
    //
    // let mut sum_weights: f64;
    // let mut dist: f64;
    // for row in 0..rows as isize {
    //     for col in 0..columns as isize {
    //         if opening[row as usize][col as usize] != nodata {
    //             if modified[row as usize][col as usize] {
    //                 sum_weights = 0f64;
    //                 let ret = frs.search(col as f64, row as f64);
    //                 for j in 0..ret.len() {
    //                     dist = ret[j].1;
    //                     if dist > 0.0 {
    //                         sum_weights += 1.0 / (dist * dist);
    //                     }
    //                 }
    //                 z = 0.0;
    //                 for j in 0..ret.len() {
    //                     dist = ret[j].1;
    //                     if dist > 0.0 {
    //                         z += ret[j].0 * (1.0 / (dist * dist)) / sum_weights;
    //                     }
    //                 }
    //                 output.set_value(row, col, -z);
    //             }
    //         }
    //     }
    //     if verbose {
    //         progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
    //         if progress != old_progress {
    //             println!("Progress: {}%", progress);
    //             old_progress = progress;
    //         }
    //     }
    // }
    //
    // let output_dem = true;
    // if output_dem {
    //     for row in 0..rows as isize {
    //         for col in 0..columns as isize {
    //             // if opening[row as usize][col as usize] != nodata {
    //             //     z = output.get_value(row, col);
    //             //     output.set_value(row, col, opening[row as usize][col as usize] + z);
    //             // }
    //             if !modified[row as usize][col as usize] {
    //                 z = output.get_value(row, col);
    //                 output.set_value(row, col, opening[row as usize][col as usize] + z);
    //             } else {
    //                 output.set_value(row, col, nodata);
    //             }
    //         }
    //         if verbose {
    //             progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
    //             if progress != old_progress {
    //                 println!("Progress: {}%", progress);
    //                 old_progress = progress;
    //             }
    //         }
    //     }
    // }

    // println!("Saving data...");
    // let _ = match output.write() {
    //     Ok(_) => println!("Output file written"),
    //     Err(e) => return Err(e),
    // };

    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct GridCell {
    // priority: isize,
    row: isize,
    column: isize,
}
