use std::io::Cursor;

use glycin_utils::*;
use gufo_common::physical_dimension::PhysicalDimensionUnit;
use tiff::encoder::{Rational, TiffValue, colortype};
use tiff::tags::ResolutionUnit;

fn write_tiff<B: ByteData, C: colortype::ColorType<Inner: bytemuck::AnyBitPattern>>(
    frame: Frame<B>,
) -> Result<Vec<u8>, ProcessError>
where
    [C::Inner]: TiffValue,
{
    let mut buf = Vec::new();

    let mut tiff_encoder =
        tiff::encoder::TiffEncoder::new(Cursor::new(&mut buf)).expected_error()?;

    let mut image_encoder = tiff_encoder
        .new_image::<C>(frame.width, frame.height)
        .expected_error()?;

    if let Some(pixel_density) = frame.details.pixel_density {
        let (pixel_density, unit) =
            if matches!(pixel_density.x().unit(), PhysicalDimensionUnit::Centimeter) {
                // Make sure that both are using the same unit
                (
                    pixel_density.convert(pixel_density.x().unit()),
                    ResolutionUnit::Centimeter,
                )
            } else {
                (
                    pixel_density.convert(PhysicalDimensionUnit::Inch),
                    ResolutionUnit::Inch,
                )
            };

        let x_rational = pixel_density.x().value_rational();
        let y_rational = pixel_density.x().value_rational();

        image_encoder.x_resolution(Rational {
            n: x_rational.numerator,
            d: x_rational.denominator,
        });

        image_encoder.y_resolution(Rational {
            n: y_rational.numerator,
            d: y_rational.denominator,
        });

        image_encoder.resolution_unit(unit);
    }

    let data = bytemuck::try_cast_slice(&*frame.texture).unwrap();
    image_encoder.write_data(data).expected_error()?;

    Ok(buf)
}

pub fn create<B: ByteData>(frame: Frame<B>) -> Result<Vec<u8>, ProcessError> {
    match frame.memory_format {
        MemoryFormat::R8g8b8 => write_tiff::<B, colortype::RGB8>(frame),
        MemoryFormat::R8g8b8a8 => write_tiff::<B, colortype::RGBA8>(frame),
        MemoryFormat::R16g16b16 => write_tiff::<B, colortype::RGB16>(frame),
        MemoryFormat::R16g16b16a16 => write_tiff::<B, colortype::RGBA16>(frame),
        MemoryFormat::R32g32b32Float => write_tiff::<B, colortype::RGB32Float>(frame),
        MemoryFormat::R32g32b32a32Float => write_tiff::<B, colortype::RGB32Float>(frame),
        _ => todo!(),
    }
}
