use anyhow::Result;
use miniquad::*;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

type Images = Vec<PathBuf>;

static TYPES: &'static [&'static str] = &["jpg", "jpeg", "png", "bmp", "tif"];

fn get_filelist() -> (Images, Option<usize>) {
    let supported_image_types = TYPES.iter().map(|s| OsStr::new(s)).collect::<Vec<&OsStr>>();

    let file = std::env::args().last().expect("no file specified");
    let file_path = Path::new(&file);
    let file_directory_path = file_path.parent().expect("invalid file path");

    let mut image_filenames = file_directory_path
        .read_dir()
        .expect("problem reading directory")
        .map(|e| e.expect("problem reading file directory at {e}").path())
        .filter(|e| {
            if let Some(ext) = e.extension() {
                supported_image_types.contains(&ext)
            } else {
                false
            }
        })
        .collect::<Vec<PathBuf>>();

    // Sort image filenames by modification date descending
    image_filenames.sort_by(|a, b| {
        b.metadata()
            .expect("error reading file metadata")
            .modified()
            .unwrap()
            .cmp(
                &a.metadata()
                    .expect("error reading file metadata")
                    .modified()
                    .unwrap(),
            )
    });

    image_filenames.iter().for_each(|f| {
        println!("Found image: {:#?}", f);
    });

    let mut inital_image = None;
    image_filenames.iter().enumerate().any(|(i, p)| {
        if file_path.cmp(p).is_eq() {
            inital_image = Some(i);
            true
        } else {
            false
        }
    });

    (image_filenames, inital_image)
}

pub const VERTEX: &str = r#"#version 100
    attribute vec2 pos;
    uniform vec2 ratio;
    varying lowp vec2 texcoord;
    void main() {
        gl_Position = vec4(pos * ratio, 0, 1);
        texcoord = vec2(max(0.0, pos.x), max(0.0, -pos.y));
    }"#;

pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 texcoord;
    uniform sampler2D tex;
    void main() {
        gl_FragColor = texture2D(tex, texcoord);
    }"#;

struct Stage {
    load: bool,

    bindings: Bindings,
    pipeline: Pipeline,
    ratio: (f32, f32),
    images: Images,
    current_image_index: usize,
}

impl Stage {
    fn new(ctx: &mut Context) -> Stage {
        let texture = Texture::empty();
        texture.set_filter(ctx, FilterMode::Linear);

        let shader = Shader::new(
            ctx,
            VERTEX,
            FRAGMENT,
            ShaderMeta {
                images: vec!["tex".to_string()],
                uniforms: UniformBlockLayout {
                    uniforms: vec![UniformDesc::new("ratio", UniformType::Float2)],
                },
            },
        )
        .unwrap();

        let vertices: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0];
        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

        let bindings = Bindings {
            vertex_buffers: vec![Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices)],
            index_buffer: Buffer::immutable(ctx, BufferType::IndexBuffer, &indices),
            images: vec![texture],
        };

        let pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[VertexAttribute::new("pos", VertexFormat::Float2)],
            shader,
        );

        let (filelist, initial) = get_filelist();

        Stage {
            load: true,
            bindings,
            pipeline,
            ratio: (0.0, 0.0),
            images: filelist,
            current_image_index: initial.unwrap_or(0),
        }
    }

    fn load_image_from_current(&mut self, ctx: &mut Context) -> Result<()> {
        // load the image
        use image::io::Reader;
        use std::fs::File;
        use std::io::BufReader;

        let path = self
            .images
            .get(self.current_image_index)
            .expect("invalid image index");
        let file = File::open(path)?;
        let reader = Reader::new(BufReader::new(file)).with_guessed_format()?;
        println!("oi {:#?}", path);

        let image = reader.decode()?.to_rgba8();

        // pump the texture with the image
        let texture = self.bindings.images.get_mut(0).unwrap();
        texture.resize(ctx, image.width(), image.height(), Some(image.as_raw()));

        // calculate ratio of the image

        Ok(())
    }

    fn calculate_ratio(&mut self, ctx: &mut Context) {
        let texture = self.bindings.images.get(0).unwrap();
        let (sw, sh) = ctx.screen_size();
        let (iw, ih) = (texture.width as f32, texture.height as f32);

        if sw > sh {
            self.ratio = ((sh / sw) * (iw / ih), 1.0);
        } else {
            self.ratio = (1.0, (sw / sh) * (ih / iw));
        }
    }

    fn next_image(&mut self) {
        println!("next_image  ");
        self.current_image_index = (self.current_image_index + 1) % self.images.len();
        self.load = true;
    }

    fn prev_image(&mut self) {
        if self.current_image_index == 0 {
            self.current_image_index = self.images.len() - 1;
        } else {
            self.current_image_index -= 1;
        }
        self.load = true;
    }
}

impl EventHandler for Stage {
    fn char_event(
        &mut self,
        _ctx: &mut Context,
        character: char,
        _keymods: KeyMods,
        _repeat: bool,
    ) {
        match character {
            'u' => self.next_image(),
            'o' => self.prev_image(),
            _ => {}
        }
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        keycode: KeyCode,
        _keymods: KeyMods,
        _repeat: bool,
    ) {
        use KeyCode::*;
        match keycode {
            Right => self.next_image(),
            Left => self.prev_image(),
            Escape => std::process::exit(0),
            _ => {}
        }
    }

    fn resize_event(&mut self, ctx: &mut Context, _: f32, _: f32) {
        self.calculate_ratio(ctx);
    }

    fn update(&mut self, _ctx: &mut Context) {}

    fn draw(&mut self, ctx: &mut Context) {
        if self.load {
            self.load_image_from_current(ctx).unwrap();
            self.calculate_ratio(ctx);
            self.load = false;
        }

        ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 0.0));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);

        ctx.apply_uniforms(&[self.ratio]);
        ctx.draw(0, 6, 1);
        ctx.end_render_pass();
        ctx.commit_frame();
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "Quad Image Viewer".to_string(),
        window_resizable: true,
        high_dpi: true,
        //fullscreen: true,
        platform: conf::Platform {
            linux_backend: conf::LinuxBackend::X11Only,
            linux_x11_gl: conf::LinuxX11Gl::GLXWithEGLFallback,
            swap_interval: None,
            framebuffer_alpha: true,
        },

        ..Default::default()
    };

    start(conf, |mut ctx| Box::new(Stage::new(&mut ctx)));
}
