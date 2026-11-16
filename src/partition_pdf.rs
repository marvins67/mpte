//! Vector PDF export using the editor's glyph positions and A4 pagination.
use crate::partition::{self, CONTENT_WIDTH, MARGIN, PAGE_GAP, PAGE_HEIGHT, PAGE_WIDTH};
use eframe::egui;
use flate2::{Compression, write::ZlibEncoder};
use pdf_writer::{Content, Filter, Pdf, Rect, Ref};
use std::{io::Write, path::Path, sync::Arc};
use ttf_parser::{Face, OutlineBuilder};

pub struct Export {
    galley: Arc<egui::Galley>,
    fonts: egui::FontDefinitions,
    separators: Vec<bool>,
    pixels_per_point: f32,
}

impl Export {
    pub fn new(
        ctx: &egui::Context,
        score: &partition::Partition,
        text: &str,
    ) -> Result<Self, String> {
        let galley = partition::paginate(ctx.fonts(|f| f.layout_job(score.layout(text))), score);
        if galley
            .rows
            .iter()
            .any(|row| row.rect.width() > CONTENT_WIDTH)
        {
            return Err("Export PDF : certaines lignes dépassent la largeur A4. Réduisez leur taille ou ajoutez un retour à la ligne avant d’exporter.".into());
        }
        Ok(Self {
            separators: (0..galley.rows.len())
                .map(|i| score.is_separator(i))
                .collect(),
            galley,
            fonts: crate::theme::font_definitions(),
            pixels_per_point: ctx.pixels_per_point(),
        })
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let count = partition::page_count(&self.galley);
        let mut pdf = Pdf::new();
        pdf.catalog(Ref::new(1)).pages(Ref::new(2));
        pdf.pages(Ref::new(2))
            .kids((0..count).map(|i| Ref::new(3 + i as i32 * 2)))
            .count(count as i32);
        let fonts: Vec<_> = self.fonts.families[&partition::font_family()]
            .iter()
            .map(|name| {
                let data = &self.fonts.font_data[name];
                Face::parse(&data.font, data.index)
                    .map(|face| (face, data.tweak.scale))
                    .map_err(|e| format!("Export PDF : police illisible : {e:?}"))
            })
            .collect::<Result<_, _>>()?;
        let mut rows = self.galley.rows.iter().enumerate().peekable();
        for page in 0..count {
            let page_id = Ref::new(3 + page as i32 * 2);
            let content_id = Ref::new(4 + page as i32 * 2);
            pdf.page(page_id)
                .parent(Ref::new(2))
                .media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT))
                .contents(content_id)
                .resources();
            let top = page as f32 * (PAGE_HEIGHT + PAGE_GAP);
            let mut content = Content::new();
            content.set_fill_rgb(36.0 / 255.0, 45.0 / 255.0, 58.0 / 255.0);
            while let Some(&(index, row)) = rows.peek() {
                if (row.rect.top() / (PAGE_HEIGHT + PAGE_GAP)).floor() as usize != page {
                    break;
                }
                rows.next();
                if self.separators[index] {
                    content.rect(
                        MARGIN,
                        PAGE_HEIGHT - MARGIN - (row.rect.center().y - top) - 0.5,
                        CONTENT_WIDTH,
                        1.0,
                    );
                    content.fill_nonzero();
                    continue;
                }
                for glyph in &row.glyphs {
                    if glyph.uv_rect.is_nothing() {
                        continue;
                    }
                    let find = |ch| {
                        fonts.iter().find_map(|(face, tweak)| {
                            face.glyph_index(ch).map(|id| (face, tweak, id))
                        })
                    };
                    let (face, tweak, id) = find(glyph.chr)
                        .or_else(|| find('◻'))
                        .or_else(|| find('?'))
                        .ok_or("Export PDF : caractère sans police disponible.")?;
                    let Some(bounds) = face.glyph_bounding_box(id) else {
                        continue;
                    };
                    let size = self.galley.job.sections[glyph.section_index as usize]
                        .format
                        .font_id
                        .size;
                    let height = f32::from(face.ascender()) - f32::from(face.descender());
                    let pixel_scale = (size * self.pixels_per_point * tweak * height
                        / f32::from(face.units_per_em()))
                    .round()
                        / height;
                    let scale = pixel_scale / self.pixels_per_point;
                    // Match the atlas's rounded ink bounds, including fallback font offsets.
                    let x = MARGIN + glyph.pos.x + glyph.uv_rect.offset.x
                        - (f32::from(bounds.x_min) * pixel_scale).floor() / self.pixels_per_point;
                    let baseline = glyph.pos.y
                        + glyph.uv_rect.offset.y
                        + (f32::from(bounds.y_max) * pixel_scale).ceil() / self.pixels_per_point;
                    let y = PAGE_HEIGHT - MARGIN - (baseline - top);
                    content
                        .save_state()
                        .transform([scale, 0.0, 0.0, scale, x, y]);
                    face.outline_glyph(
                        id,
                        &mut GlyphPath {
                            content: &mut content,
                            position: (0.0, 0.0),
                        },
                    );
                    content.fill_nonzero().restore_state();
                }
            }
            let mut compressed = ZlibEncoder::new(Vec::new(), Compression::default());
            compressed
                .write_all(&content.finish())
                .map_err(|e| e.to_string())?;
            let compressed = compressed.finish().map_err(|e| e.to_string())?;
            pdf.stream(content_id, &compressed)
                .filter(Filter::FlateDecode);
        }
        Ok(pdf.finish())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = self.bytes()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

struct GlyphPath<'a> {
    content: &'a mut Content,
    position: (f32, f32),
}

impl OutlineBuilder for GlyphPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.content.move_to(x, y);
        self.position = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.content.line_to(x, y);
        self.position = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.position;
        self.content.cubic_to(
            x0 + (x1 - x0) * 2.0 / 3.0,
            y0 + (y1 - y0) * 2.0 / 3.0,
            x + (x1 - x) * 2.0 / 3.0,
            y + (y1 - y) * 2.0 / 3.0,
            x,
            y,
        );
        self.position = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.content.cubic_to(x1, y1, x2, y2, x, y);
        self.position = (x, y);
    }
    fn close(&mut self) {
        self.content.close_path();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_frame(test: impl FnOnce(&egui::Context)) {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut test = Some(test);
        let _ = ctx.run(Default::default(), |ctx| test.take().unwrap()(ctx));
    }

    #[test]
    fn exports_current_typography_and_pages_without_changing_the_score() {
        in_frame(|ctx| {
            let mut text = format!(
                "Été — accords ♯ ♭\nAm\tG\n{}",
                "Paroles et mélodie\n".repeat(90)
            );
            let mut score = partition::Partition::new(&text);
            score.resize(0.."Été — accords ♯ ♭".len(), 10.0, false);
            score.toggle_center(0, 0..0);
            score.insert_separator(&mut text, 0, 0..0);
            let before = score.data();
            let export = Export::new(ctx, &score, &text).unwrap();
            assert!(partition::page_count(&export.galley) >= 2);
            assert!(export.galley.rows[0].rect.left() > 0.0);
            assert!(export.separators[1]);
            let bytes = export.bytes().unwrap();
            assert!(bytes.starts_with(b"%PDF-1.7"));
            assert!(bytes.ends_with(b"%%EOF"));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("partition.pdf");
            export.save(&path).unwrap();
            assert_eq!(std::fs::read(path).unwrap(), bytes);
            assert_eq!(score.data(), before);
            assert!(
                export
                    .save(&directory.path().join("missing/partition.pdf"))
                    .is_err()
            );
        });
    }

    #[test]
    fn empty_score_has_one_page_and_overlong_lines_are_rejected() {
        in_frame(|ctx| {
            for text in ["", "\n\n", "été\n🦀\n\u{10ffff}"] {
                let score = partition::Partition::new(text);
                let export = Export::new(ctx, &score, text).unwrap();
                assert_eq!(partition::page_count(&export.galley), 1);
                assert!(!export.bytes().unwrap().is_empty());
            }
            let text = "W".repeat(200);
            let score = partition::Partition::new(&text);
            assert!(Export::new(ctx, &score, &text).is_err());
        });
    }
}
