use image::GenericImageView;

pub struct Texture(u32);
impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.0);
        }
    }
}

impl Texture {
    pub fn new(im: &image::DynamicImage, _vflip: bool) -> Self {
        let im = im.flipv();
        let format = match im {
            image::ImageLuma8(_) => gl::RED,
            image::ImageLumaA8(_) => gl::RG,
            image::ImageRgb8(_) => gl::RGB,
            image::ImageRgba8(_) => gl::RGBA,
            image::ImageBgr8(_) => gl::RGB,
            image::ImageBgra8(_) => gl::RGBA,
        };
        // if vflip {
        //     im = &im.flipv();
        // }
        let data = im.raw_pixels();
        let mut texture = 0;

        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                format as i32,
                im.width() as i32,
                im.height() as i32,
                0,
                format,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as _,
            );
            gl::GenerateMipmap(gl::TEXTURE_2D);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
        Self(texture)
    }
    pub fn using(&self, f: impl FnOnce() -> ()) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.0);
            f();
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}
