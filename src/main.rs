#![windows_subsystem = "windows"]
mod textutils;

use eframe::{egui};
use eframe::egui::{IconData, ViewportBuilder};
use egui_extras;
use egui_file_dialog::FileDialog;

use resvg::tiny_skia::{Pixmap, Transform};

use std::fs::File;
use glob::glob;
use std::io::{Error, Read, Write};
use tempdir::TempDir;
use std::path::Path;
use std::path::PathBuf;
use csv::ReaderBuilder;
use lopdf::{Document, Object};
use std::collections::BTreeMap;
use std::sync::Arc;
use svg2pdf::{self, PageOptions, ConversionOptions};

struct FCDS<'a> {
    save_file_dialog: FileDialog,
    open_file_dialog: FileDialog,
    csv_file: Option<PathBuf>,
    page_width: f64,
    page_height: f64,
    num_cards_width: i32,
    num_cards_height: i32,
    flip_horizontal: bool,
    flip_vertical: bool,
    preview_page: i32,
    total_pages: i32,
    error: bool,
    generated: bool,
    header: bool,
    saved: bool,
    tmp_path: &'a Path,
}

impl eframe::App for FCDS<'_> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.error {
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("An error occurred while generating flashcards.");
                    if ui.button("Close").clicked() {
                        self.error = false;
                    }
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(path) = self.save_file_dialog.update(ctx).picked() {
                if !self.saved {
                    let path_buf = add_pdf_extension(path);
                    let _ = std::fs::copy(self.tmp_path.join("flashcards.pdf"), path_buf);
                    self.saved = true;
                }
            }
            self.open_file_dialog.update(ctx);
            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let file_picker_button = ui.button("Select CSV File");
                            if file_picker_button.clicked() {
                                self.open_file_dialog.pick_file();
                            }
                            if let Some(path) = self.open_file_dialog.take_picked() {
                                self.csv_file = Some(path.to_path_buf());
                            }
                            if let Some(path) = self.csv_file.clone() {
                                if let Result::Ok(path_string) = path.into_os_string().into_string() {
                                    ui.label(path_string);
                                }
                            }
                        });
                        if !self.csv_file.is_none() {
                            ui.checkbox(&mut self.header, "Has Header");
                            ui.label("Facade ignores the first row of the CSV if the button above is checked.");
                        }
                    });
                    if !self.csv_file.is_none() {
                        ui.separator();
                        ui.label("The following can be used if your printer\nflips pages when printing double sided:");
                        ui.checkbox(&mut self.flip_horizontal, "Flip horizontal");
                        ui.checkbox(&mut self.flip_vertical, "Flip vertical");
                        ui.separator();
                        ui.label("Set page dimensions");
                        ui.add(egui::Slider::new(&mut self.page_width, 0.0..=1200.0).text("mm  Page Width"));
                        ui.add(egui::Slider::new(&mut self.page_height, 0.0..=1200.0).text("mm  Page Height"));
                        ui.separator();
                        ui.label("Set flashcard sizes");
                        ui.add(egui::Slider::new(&mut self.num_cards_width, 0..=10).text("Flashcard Width"));
                        ui.add(egui::Slider::new(&mut self.num_cards_height, 0..=10).text("Flashcard Height"));
                        ui.separator();
                        ui.label("Standard Paper Sizes:");
                        ui.horizontal(|ui| {
                            let a4_button = ui.button("A4");
                            if a4_button.clicked() {
                                self.page_height = 297.0;
                                self.page_width = 210.0;
                            }
                            let letter_button = ui.button("Letter");
                            if letter_button.clicked() {
                                self.page_width = 215.9;
                                self.page_height = 279.4;
                            }
                            let legal_button = ui.button("Legal");
                            if legal_button.clicked() {
                                self.page_width = 215.9;
                                self.page_height = 355.6;
                            }
                            let tabloid_button = ui.button("Tabloid");
                            if tabloid_button.clicked() {
                                self.page_width = 279.0;
                                self.page_height = 432.0;
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            let gen_button = ui.button("Generate Flashcards");
                            if gen_button.clicked() {
                                self.generated = true;
                                if let Ok(total_pages) = gen_cards(self.page_width, self.page_height, self.num_cards_width, self.num_cards_height, self.flip_horizontal, self.flip_vertical, self.header, self.csv_file.clone(), self.tmp_path) {
                                    self.total_pages = total_pages;
                                } else {
                                    self.error = true;
                                }
                                if let Some(path) = self.tmp_path.join("flashcards*.png").to_str() {
                                    if let Ok(entries) = glob(path) {
                                        for entry in entries {
                                            if let Some(entry) = entry.ok() {
                                                ctx.forget_image(format!("file://{}", entry.display()).as_str());
                                            };
                                        }
                                    }
                                }
                            }
                            if self.generated {
                                let export_button = ui.button("Export");
                                if export_button.clicked() {
                                    self.saved = false;
                                    self.save_file_dialog.save_file();
                                }
                            }
                        });
                    }
                });
                columns[1].vertical(|ui| {
                    ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                    if self.generated{
                        ui.vertical(|ui| {
                            if let Some(path) = self.tmp_path.join(format!("flashcards{}.png", self.preview_page)).to_str() {
                                let image = egui::Image::new("file://".to_owned() + path).max_height(ui.available_height() - 50.0);
                                ui.add(image);
                                ui.horizontal(|ui| {
                                    let back_button = ui.button("<");
                                    if back_button.clicked() {
                                        self.preview_page = std::cmp::max(self.preview_page - 1, 0);
                                    }
                                    ui.label(format!("Page: {}", self.preview_page + 1));
                                    let back_button = ui.button(">");
                                    if back_button.clicked() {
                                        self.preview_page = std::cmp::min(self.preview_page + 1, self.total_pages - 1);
                                    }
                                });
                            }
                        });
                    }else {
                        if !self.csv_file.is_none() {
                            ui.label("Press the Generate Flashcards button for a preview");
                        }
                    }
                    });
                });
            });
        });
    }
}

fn main() {
    const ICON_BYTES: &[u8] = include_bytes!("../facadelogo.png");

    let icon_data = {
        let image = image::load_from_memory(ICON_BYTES)
            .expect("Failed to load icon")
            .into_rgba8();
        let (width, height) = image.dimensions();

        Some(IconData {
            rgba: image.into_raw(),
            width,
            height,
        })
    };

    let mut frame_options = eframe::NativeOptions::default();
    if let Some(icon_data) = icon_data {
        frame_options.viewport = ViewportBuilder {
            title: Some("Facade".to_string()),
            icon: Some(Arc::new(icon_data)),

            ..Default::default()
        }
    }
    //Create a temp dir
    let temp_dir = TempDir::new("Facade").expect("Failed to create temp dir");
    let temp_dir = temp_dir.path();
    let _ = eframe::run_native("Facade", frame_options, Box::new(|ctx| {
        egui_extras::install_image_loaders(&ctx.egui_ctx);
        Ok(Box::new(FCDS{save_file_dialog: FileDialog::new(), open_file_dialog: FileDialog::new().add_file_filter("CSV files",  Arc::new(|path| path.extension().unwrap_or_default() == "csv")).default_file_filter("CSV files"), csv_file: Option::None, page_width: 215.9, page_height: 279.4, num_cards_width: 3, num_cards_height: 4, flip_horizontal: true, flip_vertical: false, preview_page: 0, total_pages: 0, error: false, saved: false, header: true, generated: false, tmp_path: temp_dir}))
    }));

}

fn gen_cards(page_width: f64, page_height: f64, num_cards_width: i32, num_cards_height: i32, flip_horizontal: bool, flip_vertical: bool, headers: bool, csv_file: Option<PathBuf>, temp_dir: &Path) -> Result<i32, Box<dyn std::error::Error>>{
    let mut pdf_paths: Vec<String> = Vec::new();
    let svg_path = temp_dir.join("flashcards.svg");
    let mut page_num = 0;

    let Some(csv_file) = csv_file else {
        return Err("An error occurred".into())
    };
    let file = File::open(csv_file)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(headers)
        .from_reader(file);

    let mut terms = Vec::new();
    let mut definitions = Vec::new();

    for result in reader.records() {
        let record = result?;

        // Extract values from the specified columns if they exist
        if 0 < record.len() {
            terms.push(record[0].to_string());
        }

        if 1 < record.len() {
            definitions.push(record[1].to_string());
        }
    }


    while !terms.is_empty() {
        let mut svg_file = File::create(svg_path.clone())?;

        // Do the header
        write!(svg_file, "<svg width=\"{}mm\" height=\"{}mm\" version=\"1.1\" style='background-color: white;' xmlns=\"http://www.w3.org/2000/svg\">",
               page_width, page_height)?;

        // Now the vertical lines
        for i in 0..(num_cards_width - 1) {
            let line_position = (i as f64 + 1.0) * (page_width / num_cards_width as f64);
            write!(svg_file, "<line x1=\"{}mm\" y1=\"0mm\" x2=\"{}mm\" y2=\"{}mm\" stroke=\"black\" stroke-width=\"1\"/>",
                   line_position, line_position, page_height)?;
        }

        for i in 0..(num_cards_height - 1) {
            let line_position = (i as f64 + 1.0) * (page_height / num_cards_height as f64);
            write!(svg_file, "<line x1=\"0mm\" y1=\"{}mm\" x2=\"{}mm\" y2=\"{}mm\" stroke=\"black\" stroke-width=\"1\"/>",
                   line_position, page_width, line_position)?;
        }

        // Finally the text
        for i in 0..num_cards_width {
            for j in 0..num_cards_height {
                if terms.is_empty() {
                    break;
                }
                // Calculate center position
                let center_x = (i as f64 * (page_width / num_cards_width as f64) +
                    (i as f64 + 1.0) * (page_width / num_cards_width as f64)) / 2.0;
                let center_y = (j as f64 * (page_height / num_cards_height as f64) +
                    (j as f64 + 1.0) * (page_height / num_cards_height as f64)) / 2.0;

                // Write the text

                let text_element = textutils::generate_centered_text_element(
                    &terms[0],
                    center_x,
                    center_y,
                    (0.153 * page_width / num_cards_width as f64).floor() as usize,
                    30.0,
                    1.1,
                    "Arial");

                write!(svg_file, "{}", text_element)?;
                terms.remove(0);
            }
        }

        // End the svg
        write!(svg_file, "</svg>")?;
        svg_file.flush()?;


        if convert_svg_to_png(svg_path.clone(), temp_dir.join(format!("flashcards{}.png",page_num))).is_err() {
            return Err("An error occurred".into());
        }

        // Convert SVG to PDF
        let svg = std::fs::read_to_string(temp_dir.join("flashcards.svg")).unwrap();
        let mut options = svg2pdf::usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        let tree = svg2pdf::usvg::Tree::from_str(&svg, &options).unwrap();

        let pdf_data = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default()).map_err(|e| e.to_string())?;
        let pdf_path = temp_dir.join(format!("flashcards{}.pdf", page_num));
        std::fs::write(pdf_path.clone(), pdf_data).unwrap();
        if let Some(pdf_path) = pdf_path.to_str() {
            pdf_paths.push(pdf_path.to_string());
        }


        page_num += 1;

        // Now do the definitions
        let mut svg_file = File::create(svg_path.clone())?;

        // Do the header
        write!(svg_file, "<svg width=\"{}mm\" height=\"{}mm\" version=\"1.1\" style='background-color: white;' xmlns=\"http://www.w3.org/2000/svg\">",
               page_width, page_height)?;

        // Now the vertical lines
        for i in 0..(num_cards_width - 1) {
            let line_position = (i as f64 + 1.0) * (page_width / num_cards_width as f64);
            write!(svg_file, "<line x1=\"{}mm\" y1=\"0mm\" x2=\"{}mm\" y2=\"{}mm\" stroke=\"black\" stroke-width=\"1\"/>",
                   line_position, line_position, page_height)?;
        }

        for i in 0..(num_cards_height - 1) {
            let line_position = (i as f64 + 1.0) * (page_height / num_cards_height as f64);
            write!(svg_file, "<line x1=\"0mm\" y1=\"{}mm\" x2=\"{}mm\" y2=\"{}mm\" stroke=\"black\" stroke-width=\"1\"/>",
                   line_position, page_width, line_position)?;
        }
        let width_range: Box<dyn Iterator<Item = i32>> = if flip_horizontal {
            Box::new((0..num_cards_width).rev())
        } else {
            Box::new(0..num_cards_width)
        };

        // Finally the text
        for i in width_range {
            let height_range: Box<dyn Iterator<Item = i32>> = if flip_vertical {
                Box::new((0..num_cards_height).rev())
            } else {
                Box::new(0..num_cards_height)
            };
            for j in height_range {
                if definitions.is_empty() {
                    break;
                }
                // Calculate center position
                let center_x = (i as f64 * (page_width / num_cards_width as f64) +
                    (i as f64 + 1.0) * (page_width / num_cards_width as f64)) / 2.0;
                let center_y = (j as f64 * (page_height / num_cards_height as f64) +
                    (j as f64 + 1.0) * (page_height / num_cards_height as f64)) / 2.0;

                //Write the text

                let text_element = textutils::generate_centered_text_element(
                    &definitions[0],
                    center_x,
                    center_y,
                    (0.37054191755 * page_width / num_cards_width as f64).floor() as usize,
                    12.0,
                    1.1,
                    "Arial"
                );

                write!(svg_file, "{}", text_element)?;
                definitions.remove(0);
            }
        }

        // End the svg
        write!(svg_file, "</svg>").expect("Failed to write SVG closing tag");
        svg_file.flush().expect("Failed to flush SVG file");

        if convert_svg_to_png(svg_path.clone(), temp_dir.join(format!("flashcards{}.png",page_num))).is_err() {
            return Err("An error occurred".into());
        }
        // Convert SVG to PDF
        let svg = std::fs::read_to_string(temp_dir.join("flashcards.svg")).unwrap();
        let mut options = svg2pdf::usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        let tree = svg2pdf::usvg::Tree::from_str(&svg, &options).unwrap();

        let pdf_data = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default()).map_err(|e| e.to_string())?;
        let pdf_path = temp_dir.join(format!("flashcards{}.pdf", page_num));
        std::fs::write(pdf_path.clone(), pdf_data).unwrap();
        if let Some(pdf_path) = pdf_path.to_str() {
            pdf_paths.push(pdf_path.to_string());
        }
        page_num += 1;
    }
    merge_pdfs(&pdf_paths.iter().map(|x| x.as_str()).collect(), temp_dir.join("flashcards.pdf")).expect("Failed to merge PDFs");
    return Ok(page_num);
}

fn add_pdf_extension(path: &Path) -> PathBuf {

    let path_ref = path;
    // Check if the path already has a .pdf extension (case insensitive)
    if let Some(ext) = path_ref.extension() {
        if ext.to_ascii_lowercase() == "pdf" {
            return path_ref.to_path_buf();
        }
    }

    // Add .pdf extension
    let mut result = path_ref.to_path_buf();
    let new_filename = match path_ref.file_name() {
        Some(filename) => {
            let mut filename_str = filename.to_string_lossy().into_owned();
            filename_str.push_str(".pdf");
            filename_str
        },
        None => "flashcards.pdf".to_string(),
    };

    // If there was no filename component, this will just set the new filename
    result.set_file_name(new_filename);
    result
}
fn merge_pdfs(pdfs: &Vec<&str>, output_path: PathBuf) -> Result<(), Error> {
    if pdfs.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No PDF files provided for merging",
        ));
    }

    // If only one PDF, just copy it to output
    if pdfs.len() == 1 {
        let mut doc = Document::load(pdfs[0]).map_err(|e| {return std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to load PDF: {}", e))})?;
        let _ = doc.save(output_path).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to save PDF: {}", e))
        });
        return Ok(())
    }

    // Load all documents
    let mut documents = Vec::with_capacity(pdfs.len());
    for &pdf_path in pdfs.into_iter() {
        match Document::load(pdf_path) {
            Ok(doc) => documents.push(doc),
            Err(e) => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to load PDF {}: {}", pdf_path, e)
            )),
        }
    }

    // Create a new document to hold the merged result
    let mut merged_doc = Document::with_version("1.5");
    let mut max_id = 1;
    let mut all_pages = BTreeMap::new();
    let mut all_objects = BTreeMap::new();

    // Process each document
    for mut doc in documents {
        // Renumber objects for the current document
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // Add all objects from this document
        all_objects.extend(doc.objects.clone());

        // Collect all pages from this document
        for (_, object_id) in doc.get_pages() {
            if let Some(page_object) = doc.objects.get(&object_id) {
                all_pages.insert(object_id, page_object.clone());
            }
        }
    }

    // Add all collected objects to the merged document
    merged_doc.objects = all_objects;

    // Create a Pages object referencing all the collected pages
    let pages_object_id = (max_id, 0);
    max_id += 1;

    let pages_object = Object::Dictionary(
        lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name("Pages".into())),
            ("Count", Object::Integer(all_pages.len() as i64)),
            (
                "Kids",
                Object::Array(
                    all_pages
                        .keys()
                        .cloned()
                        .map(|key| Object::Reference(key))
                        .collect(),
                ),
            ),
        ]),
    );
    merged_doc.objects.insert(pages_object_id, pages_object);

    // Create a catalog object referring to the pages
    let catalog_object_id = (max_id, 0);
    merged_doc.objects.insert(
        catalog_object_id,
        Object::Dictionary(lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name("Catalog".into())),
            ("Pages", Object::Reference(pages_object_id)),
        ])),
    );

    // Update the document trailer and metadata
    merged_doc.trailer.set("Root", Object::Reference(catalog_object_id));
    merged_doc.max_id = max_id;

    // Optimize the document
    merged_doc.renumber_objects();
    merged_doc.compress();

    // Save the merged document
    let _ = merged_doc.save(output_path).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to save merged PDF: {}", e))
    });
    Ok(())
}


fn convert_svg_to_png(svg_path: PathBuf, png_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Read SVG file
    let mut svg_file = File::open(svg_path)?;
    let mut svg_content = String::new();
    svg_file.read_to_string(&mut svg_content)?;

    let mut fontdb = svg2pdf::usvg::fontdb::Database::new();
    fontdb.load_system_fonts();

    let fontdb_arc = Arc::new(fontdb);

    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb = fontdb_arc.clone();
    // Parse SVG using usvg::Tree::from_str
    let tree = svg2pdf::usvg::Tree::from_str(
        &svg_content,
        &opt
    )?;


    // Create a pixmap with the SVG's size
    let size = tree.size();
    let mut pixmap = Pixmap::new(
        size.width() as u32,
        size.height() as u32
    ).ok_or("Failed to create pixmap")?;

    // Render the SVG to the pixmap
    resvg::render(
        &tree,
        Transform::default(),
        &mut pixmap.as_mut()
    );

    // Save the pixmap as PNG
    let png_data = pixmap.encode_png()?;
    let mut png_file = File::create(png_path)?;
    png_file.write_all(&png_data)?;

    Ok(())
}



