use rah_api::tools::{MAX_IMAGE_DATA, describe_image, image};
use rah_protocol::{ImageContent, ToolContent};

#[test]
fn image_accepts_known_types_and_padded_base64() {
    assert_eq!(
        image(" Image/PNG ", "iVBORw0K"),
        Ok(ToolContent::Image(ImageContent {
            media_type: "image/png".into(),
            data: "iVBORw0K".into(),
        }))
    );
    for data in ["AA==", "AAA=", "/+9a"] {
        assert!(image("image/jpeg", data).is_ok(), "{data}");
    }
}

#[test]
fn image_rejects_other_types_bad_base64_and_large_data() {
    for media_type in ["image/svg+xml", "text/html", ""] {
        assert!(image(media_type, "AA==").is_err(), "{media_type}");
    }
    for data in [
        "",
        "AA=",
        "A===",
        "AA=A",
        "AA-_",
        "data:image/png;base64,AA==",
    ] {
        assert!(image("image/png", data).is_err(), "{data:?}");
    }
    assert!(image("image/png", &"A".repeat(MAX_IMAGE_DATA)).is_ok());
    assert!(image("image/png", &"A".repeat(MAX_IMAGE_DATA + 4)).is_err());
}

#[test]
fn describe_image_names_the_type_and_size() {
    let small = ImageContent {
        media_type: "image/png".into(),
        data: "A".repeat(4000),
    };
    assert_eq!(describe_image(&small), "[image/png image, 3 KiB]");
    let large = ImageContent {
        media_type: "image/webp".into(),
        data: "A".repeat(4 * 1024 * 1024),
    };
    assert_eq!(describe_image(&large), "[image/webp image, 3.0 MiB]");
}
