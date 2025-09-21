use crate::{MemoryFormat, MemoryFormatInfo};

/// Selection of memory formats the API user accepts
#[cfg(feature = "gobject")]
#[glib::flags(name = "GlyMemoryFormatSelection")]
pub enum MemoryFormatSelection {
    B8g8r8a8Premultiplied = (1 << 0),
    A8r8g8b8Premultiplied = (1 << 1),
    R8g8b8a8Premultiplied = (1 << 2),
    B8g8r8a8 = (1 << 3),
    A8r8g8b8 = (1 << 4),
    R8g8b8a8 = (1 << 5),
    A8b8g8r8 = (1 << 6),
    R8g8b8 = (1 << 7),
    B8g8r8 = (1 << 8),
    R16g16b16 = (1 << 9),
    R16g16b16a16Premultiplied = (1 << 10),
    R16g16b16a16 = (1 << 11),
    R16g16b16Float = (1 << 12),
    R16g16b16a16Float = (1 << 13),
    R32g32b32Float = (1 << 14),
    R32g32b32a32FloatPremultiplied = (1 << 15),
    R32g32b32a32Float = (1 << 16),
    G8a8Premultiplied = (1 << 17),
    G8a8 = (1 << 18),
    G8 = (1 << 19),
    G16a16Premultiplied = (1 << 20),
    G16a16 = (1 << 21),
    G16 = (1 << 22),
}

#[cfg(not(feature = "gobject"))]
bitflags::bitflags! {
    /// Selection of memory formats the API user accepts
    #[derive(Debug, Clone, Copy)]
    pub struct MemoryFormatSelection: u32 {
        const B8g8r8a8Premultiplied = (1 << 0);
        const A8r8g8b8Premultiplied = (1 << 1);
        const R8g8b8a8Premultiplied = (1 << 2);
        const B8g8r8a8 = (1 << 3);
        const A8r8g8b8 = (1 << 4);
        const R8g8b8a8 = (1 << 5);
        const A8b8g8r8 = (1 << 6);
        const R8g8b8 = (1 << 7);
        const B8g8r8 = (1 << 8);
        const R16g16b16 = (1 << 9);
        const R16g16b16a16Premultiplied = (1 << 10);
        const R16g16b16a16 = (1 << 11);
        const R16g16b16Float = (1 << 12);
        const R16g16b16a16Float = (1 << 13);
        const R32g32b32Float = (1 << 14);
        const R32g32b32a32FloatPremultiplied = (1 << 15);
        const R32g32b32a32Float = (1 << 16);
        const G8a8Premultiplied = (1 << 17);
        const G8a8 = (1 << 18);
        const G8 = (1 << 19);
        const G16a16Premultiplied = (1 << 20);
        const G16a16 = (1 << 21);
        const G16 = (1 << 22);
    }
}

impl Default for MemoryFormatSelection {
    fn default() -> Self {
        Self::all()
    }
}

impl MemoryFormatSelection {
    const X: [(MemoryFormatSelection, MemoryFormat); 23] = [
        (
            MemoryFormatSelection::B8g8r8a8Premultiplied,
            MemoryFormat::B8g8r8a8Premultiplied,
        ),
        (
            MemoryFormatSelection::A8r8g8b8Premultiplied,
            MemoryFormat::A8r8g8b8Premultiplied,
        ),
        (
            MemoryFormatSelection::R8g8b8a8Premultiplied,
            MemoryFormat::R8g8b8a8Premultiplied,
        ),
        (MemoryFormatSelection::B8g8r8a8, MemoryFormat::B8g8r8a8),
        (MemoryFormatSelection::A8r8g8b8, MemoryFormat::A8r8g8b8),
        (MemoryFormatSelection::R8g8b8a8, MemoryFormat::R8g8b8a8),
        (MemoryFormatSelection::A8b8g8r8, MemoryFormat::A8b8g8r8),
        (MemoryFormatSelection::R8g8b8, MemoryFormat::R8g8b8),
        (MemoryFormatSelection::B8g8r8, MemoryFormat::B8g8r8),
        (MemoryFormatSelection::R16g16b16, MemoryFormat::R16g16b16),
        (
            MemoryFormatSelection::R16g16b16a16Premultiplied,
            MemoryFormat::R16g16b16a16Premultiplied,
        ),
        (
            MemoryFormatSelection::R16g16b16a16,
            MemoryFormat::R16g16b16a16,
        ),
        (
            MemoryFormatSelection::R16g16b16Float,
            MemoryFormat::R16g16b16Float,
        ),
        (
            MemoryFormatSelection::R16g16b16a16Float,
            MemoryFormat::R16g16b16a16Float,
        ),
        (
            MemoryFormatSelection::R32g32b32Float,
            MemoryFormat::R32g32b32Float,
        ),
        (
            MemoryFormatSelection::R32g32b32a32FloatPremultiplied,
            MemoryFormat::R32g32b32a32FloatPremultiplied,
        ),
        (
            MemoryFormatSelection::R32g32b32a32Float,
            MemoryFormat::R32g32b32a32Float,
        ),
        (
            MemoryFormatSelection::G8a8Premultiplied,
            MemoryFormat::G8a8Premultiplied,
        ),
        (MemoryFormatSelection::G8a8, MemoryFormat::G8a8),
        (MemoryFormatSelection::G8, MemoryFormat::G8),
        (
            MemoryFormatSelection::G16a16Premultiplied,
            MemoryFormat::G16a16Premultiplied,
        ),
        (MemoryFormatSelection::G16a16, MemoryFormat::G16a16),
        (MemoryFormatSelection::G16, MemoryFormat::G16),
    ];

    /// List of selected memory formats
    pub fn memory_formats(self) -> Vec<MemoryFormat> {
        let mut vec = Vec::new();
        for (selection, format) in Self::X {
            if self.contains(selection) {
                vec.push(format);
            }
        }

        vec
    }

    /// Select the best contained format to represent `src`
    ///
    /// The function returns `None` if no formats are selected.
    ///
    /// ```
    /// # use glycin_common::{MemoryFormatSelection, MemoryFormat};
    ///
    /// assert_eq!(
    ///     (MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R8g8b8a8)
    ///         .best_format_for(MemoryFormat::A8b8g8r8),
    ///     Some(MemoryFormat::R8g8b8a8)
    /// );
    ///
    /// assert_eq!(
    ///     (MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R8g8b8a8)
    ///         .best_format_for(MemoryFormat::B8g8r8),
    ///     Some(MemoryFormat::R8g8b8)
    /// );
    ///
    /// assert_eq!(
    ///     (MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R16g16b16)
    ///         .best_format_for(MemoryFormat::B8g8r8),
    ///     Some(MemoryFormat::R8g8b8)
    /// );
    ///
    /// assert_eq!(
    ///     (MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R16g16b16)
    ///         .best_format_for(MemoryFormat::R16g16b16Float),
    ///     Some(MemoryFormat::R16g16b16)
    /// );
    ///
    /// assert_eq!(
    ///     MemoryFormatSelection::empty().best_format_for(MemoryFormat::R16g16b16Float),
    ///     None
    /// );
    /// ```
    pub fn best_format_for(self, src: MemoryFormat) -> Option<MemoryFormat> {
        let formats: Vec<MemoryFormat> = self.memory_formats();

        // Shortcut if format itself is supported
        if formats.contains(&src) {
            return Some(src);
        }

        let mut formats_categorized = formats
            .into_iter()
            .map(|x| {
                (
                    // Prioritize formats by how good they can represent the original format
                    (
                        x.has_alpha() == src.has_alpha(),
                        x.n_channels() >= src.n_channels(),
                        x.channel_type() == src.channel_type(),
                        x.channel_type().size() >= src.channel_type().size(),
                        // Don't have unnecessary many channels
                        -(x.n_channels() as i8),
                        // Don't have unnecessary large types
                        -(x.channel_type().size() as i8),
                    ),
                    x,
                )
            })
            .collect::<Vec<_>>();

        formats_categorized.sort_by_key(|x| x.0);

        // The best format is the highest ranked, i.e. the last one
        formats_categorized.last().map(|x| x.1)
    }
}
