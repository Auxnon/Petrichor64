use image::GenericImageView;
pub fn load_img(str: String) {
    let text = format!("assets/{}.png", str);
    //Path::new(".").join("entities");
    let img = image::open(text).unwrap();
    // The dimensions method returns the images width and height.
    println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    println!("{:?}", img.color());
}
