use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use wgpu::{Queue, Texture};
pub fn load_img(str: String) -> DynamicImage {
    let text = format!("assets/{}.png", str);
    //Path::new(".").join("entities");
    let img = image::open(text).unwrap();
    // The dimensions method returns the images width and height.
    println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    println!("{:?}", img.color());
    img
}
pub fn load_tex(device: &wgpu::Device, queue: &Queue, str: String) -> (Texture, DynamicImage) {
    let img = load_img(str);

    let rgba = img.as_rgba8().unwrap();
    let dimensions = img.dimensions();
    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: texture_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("diffuse_texture"),
    });
    queue.write_texture(
        // Tells wgpu where to copy the pixel data
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        rgba,
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
            rows_per_image: std::num::NonZeroU32::new(dimensions.1),
        },
        texture_size,
    );
    (tex, img)
}
