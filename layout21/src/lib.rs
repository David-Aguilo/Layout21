use std::collections::HashMap;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SlotMap};

// Create key-types for each internal type stored in [SlotMap]s
new_key_type! {
    /// Keys for [Cell] entries
    pub struct CellKey;
    /// Keys for [abstrakt::Abstract] entries
    pub struct AbstractKey;
    /// Keys for [CellView] entries
    pub struct CellViewKey;
}

pub type LayoutResult<T> = Result<T, LayoutError>;

/// Distance Units Enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Unit {
    /// Micrometers, or microns for we olde folke
    Micro,
    /// Nanometers
    Nano,
}
impl Default for Unit {
    /// Default units are nanometers
    fn default() -> Unit {
        Unit::Nano
    }
}
/// Direction Enumeration
/// Primarily for [Layer] orientations
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Dir {
    /// Horizontal
    Horiz,
    /// Vertical
    Vert,
}
impl Dir {
    /// Whichever direction we are, return the other one.
    fn other(&self) -> Self {
        match self {
            Self::Horiz => Self::Vert,
            Self::Vert => Self::Horiz,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackEntry {
    pub width: usize,
    pub ttype: TrackType,
}
impl TrackEntry {
    /// Helper method: create of [TrackEntry] of [TrackType] [TrackType::Gap]
    pub fn gap(width: usize) -> Self {
        TrackEntry {
            width,
            ttype: TrackType::Gap,
        }
    }
    /// Helper method: create of [TrackEntry] of [TrackType] [TrackType::Signal]
    pub fn sig(width: usize) -> Self {
        TrackEntry {
            width,
            ttype: TrackType::Signal,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TrackType {
    Gap,
    Signal,
    Rail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrackSpec {
    Entry(TrackEntry),
    Pat(Pattern),
}
impl TrackSpec {
    pub fn gap(width: usize) -> Self {
        Self::Entry(TrackEntry {
            width,
            ttype: TrackType::Gap,
        })
    }
    pub fn sig(width: usize) -> Self {
        Self::Entry(TrackEntry {
            width,
            ttype: TrackType::Signal,
        })
    }
    pub fn rail(width: usize) -> Self {
        Self::Entry(TrackEntry {
            width,
            ttype: TrackType::Rail,
        })
    }
    pub fn pat(e: impl Into<Vec<TrackEntry>>, nrep: usize) -> Self {
        Self::Pat(Pattern::new(e, nrep))
    }
}
/// An array of layout `Entries`, repeated `nrep` times
#[derive(Default, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Pattern {
    pub entries: Vec<TrackEntry>,
    pub nrep: usize,
}
impl Pattern {
    pub fn new(e: impl Into<Vec<TrackEntry>>, nrep: usize) -> Self {
        Self {
            entries: e.into(),
            nrep,
        }
    }
}
/// # Stack
///
/// The z-stack, primarily including metal, via, and primitive layers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stack {
    /// Measurement units
    pub units: Unit,
    /// Primitive cell horizontal unit-pitch, denominated in `units`
    pub xpitch: usize,
    /// Primitive cell vertical unit-pitch, denominated in `units`
    pub ypitch: usize,
    /// Layer used for cell outlines/ boundaries
    pub boundary_layer: Option<raw::DataTypeMap>,
    /// Set of metal layers
    pub layers: Vec<Layer>,
    /// Set of via layers
    pub vias: Vec<ViaLayer>,
}
/// # Layer
///
/// Metal layer in a [Stack]
/// Each layer is effectively infinite-spanning in one dimension, and periodic in the other.
/// Layers with `dir=Dir::Horiz` extend to infinity in x, and repeat in y, and vice-versa.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Layer Index
    pub index: usize,
    /// Layer Name
    pub name: String,
    /// Direction Enumeration (Horizontal/ Vertical)
    pub dir: Dir,
    /// Default size of wire-cuts
    pub cutsize: usize,
    /// Track Size & Type Entries
    pub entries: Vec<TrackSpec>,
    /// Offset, in our periodic dimension
    pub offset: isize,
    /// Layer for streaming exports
    pub stream_layer: Option<raw::LayerSpec>,
    pub raw: Option<raw::DataTypeMap>,
}
impl Layer {
    /// Convert this [Layer]'s track-info into a [TrackPeriod]
    fn to_track_period(&self, stop: usize) -> TrackPeriod {
        let mut cursor = self.offset;
        let mut period = TrackPeriod {
            rails: Vec::new(),
            signals: Vec::new(),
        };
        for e in self.flat_entries().iter() {
            let d = &e.width;
            match e.ttype {
                TrackType::Gap => (),
                TrackType::Rail => {
                    period.rails.push(Track {
                        layer: self,
                        ttype: e.ttype,
                        index: period.rails.len(),
                        dir: self.dir,
                        start: cursor,
                        width: *d,
                        segments: vec![TrackSegment {
                            net: Some("POWER_OR_GROUND_HERE_BRO".to_string()), // FIXME!
                            start: 0,
                            stop,
                        }],
                    });
                }
                TrackType::Signal => {
                    period.signals.push(Track {
                        layer: self,
                        ttype: e.ttype,
                        index: period.signals.len(),
                        dir: self.dir,
                        start: cursor,
                        width: *d,
                        segments: vec![TrackSegment {
                            net: None,
                            start: 0,
                            stop,
                        }],
                    });
                }
            };
            cursor += *d as isize;
        }
        period
    }
    /// Convert to a vector of [Track]s
    fn tracks(&self) -> Vec<Track> {
        let mut cursor = self.offset;
        let mut index = 0;
        let mut tracks: Vec<Track> = Vec::new();
        for e in self.flat_entries().iter() {
            let d = &e.width;
            match e.ttype {
                TrackType::Gap => cursor += *d as isize,
                TrackType::Rail | TrackType::Signal => {
                    // FIXME: initial segments
                    // FIXME: nets for Pwr/Gnd
                    tracks.push(Track {
                        layer: self,
                        ttype: e.ttype,
                        index,
                        dir: self.dir,
                        start: cursor,
                        width: *d,
                        segments: Vec::new(),
                    });
                    cursor += *d as isize;
                    index += 1;
                }
            }
        }
        tracks
    }
    /// Count the number of signal-tracks per period
    fn num_signal_tracks(&self) -> usize {
        let mut n = 0;
        for e in self.flat_entries().iter() {
            match e.ttype {
                TrackType::Signal => n += 1,
                _ => (),
            };
        }
        n
    }
    /// Find the center-coordinate of the `idx`th signal track
    /// FIXME: probably move to [TrackPeriod]
    fn signal_track_center(&self, idx: usize) -> isize {
        let mut cursor = self.offset;
        let idx_mod_tracks = idx % self.num_signal_tracks(); // FIXME: num tracks
        cursor += (self.pitch() * idx / self.num_signal_tracks()) as isize;
        let mut index = 0;
        for e in self.flat_entries().iter() {
            let d = &e.width;
            match e.ttype {
                TrackType::Rail | TrackType::Gap => cursor += *d as isize,
                TrackType::Signal => {
                    if index > idx_mod_tracks {
                        panic!("COULDNT FIND TRACK SOMEHOW");
                    } else if index == idx_mod_tracks {
                        return cursor + *d as isize / 2;
                    }
                    cursor += *d as isize;
                    index += 1;
                }
            }
        }
        panic!("COULDNT FIND TRACK SOMEHOW");
    }
    /// Flatten our [Entry]s into a vector
    /// Removes any nested patterns
    fn flat_entries(&self) -> Vec<TrackEntry> {
        let mut v: Vec<TrackEntry> = Vec::new();
        for e in self.entries.iter() {
            match e {
                TrackSpec::Entry(ee) => v.push(ee.clone()),
                TrackSpec::Pat(p) => {
                    for _i in 0..p.nrep {
                        for ee in p.entries.iter() {
                            v.push(ee.clone());
                        }
                    }
                }
            }
        }
        v
    }
    /// Sum up this [Layer]'s pitch
    fn pitch(&self) -> usize {
        self.flat_entries().iter().map(|e| e.width).sum()
    }
}
/// # Via / Insulator Layer Between Metals
///
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ViaLayer {
    /// Layer index
    pub index: usize,
    /// Layer name
    pub name: String,
    /// Connected metal-layer indices
    pub between: (usize, usize),
    /// Via size
    pub size: Point,
    /// Stream-out layer numbers
    pub stream_layer: Option<raw::LayerSpec>,
}
/// Assignment of a net onto a track-intersection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assign {
    /// Net Name
    pub net: String,
    /// Track Intersection Location
    pub at: TrackIntersection,
}
/// Relative Z-Axis Reference to one Layer `Above` or `Below` another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelZ {
    Above,
    Below,
}
/// Instance of another Cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    /// Instance Name
    pub inst_name: String,
    /// Cell Name/ Path
    pub cell_name: String,
    /// Cell Definition Reference
    pub cell: CellRef,
    /// Bottom-Left Corner Point
    pub p0: Point,
    /// Reflection
    pub reflect: bool,
    /// Angle of Rotation (Degrees)
    pub angle: Option<f64>,
}
/// # Layout Library
///
/// A combination of cell definitions, sub-libraries, and metadata
///
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    /// Library Name
    pub name: String,
    /// Reference to the z-stack
    pub stack: Stack,
    /// Cell Names
    pub cell_names: Vec<String>,
    /// Abstracts
    pub abstracts: SlotMap<AbstractKey, abstrakt::Abstract>,
    /// Cell Implementations
    pub cells: SlotMap<CellKey, Cell>,
    /// Sub-Libraries
    pub libs: Vec<Library>,
}
impl Library {
    /// Create a new and initially empty [Library]
    pub fn new(name: impl Into<String>, stack: Stack) -> Self {
        Self {
            name: name.into(),
            stack,
            cell_names: Vec::new(),
            abstracts: SlotMap::with_key(),
            cells: SlotMap::with_key(),
            libs: Vec::new(),
        }
    }
    /// Convert to a [raw::Library]
    pub fn to_raw(self) -> Result<raw::Library, LayoutError> {
        RawConverter::convert(self)
    }
}
#[derive(Debug, Clone)]
pub struct Track<'a> {
    /// Layer Index
    pub layer: &'a Layer,
    /// Track Type (Rail, Signal)
    pub ttype: TrackType,
    /// Track Index
    pub index: usize,
    /// Direction
    pub dir: Dir,
    /// Starting-point in off-dir axis
    pub start: isize,
    /// Track width
    pub width: usize,
    /// Set of wire-segments, in positional order
    pub segments: Vec<TrackSegment>,
}
impl<'a> Track<'a> {
    /// Retrieve a (mutable) reference to the segment at cross-dimension `dist`
    /// Returns None for `dist` outside the segment, or in-between segments
    pub fn segment_at(&mut self, dist: isize) -> Option<&mut TrackSegment> {
        if self.segments.len() < 1 {
            return None;
        }
        for seg in self.segments.iter_mut() {
            if seg.start as isize > dist {
                return None;
            }
            if seg.start as isize <= dist && seg.stop as isize >= dist {
                return Some(seg);
            }
        }
        None
    }
    /// Cut all of our segments from `start` to `stop`
    pub fn cut(&mut self, start: usize, stop: usize) -> LayoutResult<()> {
        if self.segments.len() == 0 || stop <= start {
            return Err(LayoutError::msg("Error Cutting Track"));
        }
        // Find the segment to be cut
        let mut to_be_removed: Vec<usize> = Vec::new();
        let mut to_be_inserted: Option<(usize, TrackSegment)> = None;
        for (idx, seg) in self.segments.iter_mut().enumerate() {
            if seg.start > stop {
                // Loop done case
                break;
            } else if stop < seg.start {
                // Uninvolved, carry on
                continue;
            } else if start <= seg.start && stop >= seg.stop {
                // Removal case; cut covers entire segment
                to_be_removed.push(idx);
            } else if start > seg.start && stop >= seg.stop {
                // Stop-side trim case
                seg.stop = start;
            } else if start <= seg.start && stop < seg.stop {
                // Start-side trim case
                seg.start = stop;
            } else if start > seg.start && stop < seg.stop {
                // Internal cut case
                let mut new_seg = seg.clone();
                new_seg.stop = start;
                seg.start = stop;
                to_be_inserted = Some((idx, new_seg));
            } else {
                return Err(LayoutError::msg("Internal Error: Track::cut"));
            }
        }
        if let Some((idx, seg)) = to_be_inserted {
            self.segments.insert(idx, seg);
        } else {
            // Remove any fully-cut elements, in reverse order so as to not screw up indices
            for idx in to_be_removed.iter().rev() {
                self.segments.remove(*idx);
            }
        }
        Ok(())
    }
    /// Set the stop position for our last [TrackSegment] to `stop`
    pub fn stop(&mut self, stop: usize) -> LayoutResult<()> {
        if self.segments.len() == 0 {
            return Err(LayoutError::msg("Error Stopping Track"));
        }
        let idx = self.segments.len() - 1;
        self.segments[idx].stop = stop;
        Ok(())
    }
}
/// Transformed single period of [Track]s on a [Layer]
/// Splits track-info between signals and rails.
/// Stores each as a [Track] struct, which moves to a (start, width) size-format,
/// and includes a vector of track-segments for cutting and assigning nets.
#[derive(Debug, Clone)]
pub struct TrackPeriod<'a> {
    pub signals: Vec<Track<'a>>,
    pub rails: Vec<Track<'a>>,
}
impl<'a> TrackPeriod<'a> {
    /// Set the stop position for all [Track]s to `stop`
    pub fn stop(&mut self, stop: usize) -> LayoutResult<()> {
        for t in self.rails.iter_mut() {
            t.stop(stop)?;
        }
        for t in self.signals.iter_mut() {
            t.stop(stop)?;
        }
        Ok(())
    }
    /// Cut all [Track]s from `start` to `stop`,
    /// cutting, shortening, or deleting `segments` along the way
    pub fn cut(&mut self, start: usize, stop: usize) -> LayoutResult<()> {
        for t in self.rails.iter_mut() {
            t.cut(start, stop)?;
        }
        for t in self.signals.iter_mut() {
            t.cut(start, stop)?;
        }
        Ok(())
    }
}
/// # Segments of un-split, single-net wire on a [Track]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSegment {
    /// Net Name
    pub net: Option<String>,
    /// Start Location, in [Stack]'s `units`
    pub start: usize,
    /// End/Stop Location, in [Stack]'s `units`
    pub stop: usize,
}
/// Intersection Between Adjacent Layers in [Track]-Space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackIntersection {
    /// Layer Index
    pub layer: usize,
    /// Track Index
    pub track: usize,
    /// Intersecting Track Index
    pub at: usize,
    /// Whether `at` refers to the track-indices above or below
    pub relz: RelZ,
}
/// # Layout Cell
///
/// A combination of lower-level cell instances and net-assignments to tracks.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    /// Cell Name
    pub name: String,
    /// Top-layer index
    pub top_layer: usize,
    /// Outline shape, counted in x and y pitches of `stack`
    pub outline: Outline,
    /// Layout Instances
    pub instances: Vec<Instance>,
    /// Net-to-track assignments
    pub assignments: Vec<Assign>,
    /// Track cuts
    pub cuts: Vec<TrackIntersection>,
}
/// Block Outlines are "Tetris Shaped" rectilinear polygons
///
/// These boundaries are closed, consist solely of 90-degree rectangular turns,
/// and are specified by a counter-clockwise set of points.
/// "Holes" such as the shapes "O" and "8" and "divots" such as the shapes "U" and "H" are not supported.
///
/// Two equal-length vectors `x` and `y` describe an Outline's points.
/// Counter-clockwise-ness and divot-free-ness requires that:
/// * (a) `x` values are monotonically non-increasing, and
/// * (b) `y` values are monotonically non-decreasing
///
/// In point-space terms, such an outline has vertices at:
/// `[(0,0), (x[0], 0), (x[0], y[0]), (x[1], y[0]), ... , (0, y[-1]), (0,0)]`
/// With the final point at (0, y[-1]), and its connection back to the origin both implied.
///
/// Example: a rectangular Outline would require a single entry for each of `x` and `y`,
/// at the rectangle's vertex opposite the origin in both axes.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outline {
    pub x: Vec<usize>,
    pub y: Vec<usize>,
}
impl Outline {
    /// Outline constructor, with inline checking for validity of `x` & `y` vectors
    pub fn new(x: Vec<usize>, y: Vec<usize>) -> Result<Self, LayoutError> {
        // Check that x and y are of compatible lengths
        if x.len() != y.len() {
            return Err(LayoutError::Tbd);
        }
        if x.len() < 1 {
            return Err(LayoutError::Tbd);
        }
        // Check for x non-increasing-ness
        for k in 1..x.len() {
            if x[k] > x[k - 1] {
                return Err(LayoutError::Tbd);
            }
        }
        // Check for y non-decreasing-ness
        for k in 1..y.len() {
            if y[k] < y[k - 1] {
                return Err(LayoutError::Tbd);
            }
        }
        Ok(Self { x, y })
    }
    /// Create a new rectangular outline of dimenions `x` by `y`
    pub fn rect(x: usize, y: usize) -> Result<Self, LayoutError> {
        Self::new(vec![x], vec![y])
    }
    /// Maximum x-coordinate
    /// (Which is also always the *first* x-coordinate)
    pub fn xmax(&self) -> usize {
        self.x[0]
    }
    /// Maximum y-coordinate
    /// (Which is also always the *last* y-coordinate)
    pub fn ymax(&self) -> usize {
        self.y[self.y.len() - 1]
    }
    /// Maximum coordinate in [Dir] `dir`
    pub fn max(&self, dir: Dir) -> usize {
        match dir {
            Dir::Horiz => self.xmax(),
            Dir::Vert => self.ymax(),
        }
    }
    /// Convert to a vector of polygon-vertex Points
    pub fn points(&self) -> Vec<Point> {
        let mut pts = vec![Point { x: 0, y: 0 }];
        let mut xp: isize;
        let mut yp: isize = 0;
        for i in 0..self.x.len() {
            xp = self.x[i] as isize;
            pts.push(Point::new(xp, yp));
            yp = self.y[i] as isize;
            pts.push(Point::new(xp, yp));
        }
        // Add the final implied Point at (x, y[-1])
        pts.push(Point::new(0, yp));
        pts
    }
}
/// # Point in two-dimensional layout-space
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Point {
    x: isize,
    y: isize,
}
impl Point {
    /// Create a new [Point] from (x,y) coordinates
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }
    /// Create a new [Point] which serves as an offset in direction `dir`
    pub fn offset(val: isize, dir: Dir) -> Self {
        match dir {
            Dir::Horiz => Self { x: val, y: 0 },
            Dir::Vert => Self { x: 0, y: val },
        }
    }
    /// Create a new point shifted by `x` in the x-dimension and by `y` in the y-dimension
    pub fn shift(&self, p: &Point) -> Point {
        Point {
            x: p.x + self.x,
            y: p.y + self.y,
        }
    }
    /// Create a new point scaled by `x` in the x-dimension and by `y` in the y-dimension
    pub fn scale(&self, x: isize, y: isize) -> Point {
        Point {
            x: x * self.x,
            y: y * self.y,
        }
    }
    /// Get the coordinate associated with direction `dir`
    fn coord(&self, dir: Dir) -> isize {
        match dir {
            Dir::Horiz => self.x,
            Dir::Vert => self.y,
        }
    }
}

/// # "Raw" Layout Module
pub mod raw {
    use super::*;
    use gds21;

    // FIXME: need something like raw::Abstract, representing arbitrary-shaped abstract layouts
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Abstract;

    /// # Raw Layout Library  
    /// A collection of cell-definitions and sub-library definitions
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Library {
        /// Library Name
        pub name: String,
        /// Distance Units
        pub units: Unit,
        /// Sub-Library Definitions
        pub libs: Vec<Library>,
        /// Cell Definitions
        pub cells: Vec<Cell>,
    }
    impl Library {
        /// Create a new and empty Library
        pub fn new(name: impl Into<String>, units: Unit) -> Self {
            Self {
                name: name.into(),
                units,
                ..Default::default()
            }
        }
        /// Convert to a GDSII [gds21::GdsLibrary]
        pub fn to_gds(self) -> Result<gds21::GdsLibrary, LayoutError> {
            GdsConverter::convert(self)
        }
    }
    /// Raw-Layout Cell Definition
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Cell {
        /// Cell Name
        pub name: String,
        /// Cell Instances
        pub insts: Vec<Instance>,
        /// Instance Arrays
        pub arrays: Vec<InstArray>,
        /// Primitive Elements
        pub elems: Vec<Element>,
    }
    /// # Array of Instances
    ///
    /// Two-dimensional array of identical [Instance]s of the same [Cell].
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct InstArray {
        pub inst_name: String,
        pub cell_name: String,
        pub rows: usize,
        pub cols: usize,
        pub xpitch: usize,
        pub ypitch: usize,
        pub p0: Point,
        pub reflect: bool,
        pub angle: Option<f64>,
    }
    /// # Layer Specification
    /// As in seemingly every layout system, this uses two numbers to identify each layer.
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct LayerSpec(i16, i16);
    impl LayerSpec {
        pub fn new(n1: i16, n2: i16) -> Self {
            Self(n1, n2)
        }
    }
    /// # Per-Layer Datatype Specification
    /// Includes the datatypes used for each category of element on layer `layernum`
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DataTypeMap {
        /// Layer Number
        pub layernum: i16,
        /// Drawing (Geometry) DataType Value
        pub drawing: Option<i16>,
        /// Text DataType Value
        pub text: Option<i16>,
        /// Any Other DataType Values
        pub other: HashMap<String, i16>,
    }
    /// # Primitive Geometric Element
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Element {
        /// Net Name
        pub net: Option<String>,
        /// Layer
        pub layer: DataTypeMap,
        /// Shape
        pub inner: Shape,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Shape {
        Rect { p0: Point, p1: Point },
        Poly { pts: Vec<Point> },
    }
    impl Shape {
        /// Retrieve our "origin", or first [Point]
        pub fn point0(&self) -> &Point {
            match *self {
                Shape::Rect { ref p0, ref p1 } => p0,
                Shape::Poly { ref pts } => &pts[0],
            }
        }
        /// Calculate our center-point
        pub fn center(&self) -> Point {
            match *self {
                Shape::Rect { ref p0, ref p1 } => Point::new((p0.x + p1.x) / 2, (p0.y + p1.y) / 2),
                Shape::Poly { ref pts } => {
                    unimplemented!("Shape::Poly::center");
                }
            }
        }
        /// Indicate whether this shape is (more or less) horizontal or vertical
        pub fn orientation(&self) -> Dir {
            match *self {
                Shape::Rect { ref p0, ref p1 } => {
                    if (p1.x - p0.x).abs() < (p1.y - p0.y).abs() {
                        return Dir::Vert;
                    }
                    Dir::Horiz
                }
                Shape::Poly { ref pts } => {
                    unimplemented!("Shape::Poly::orientation");
                }
            }
        }
        /// Shift coordinates by the (x,y) values specified in `pt`
        pub fn shift(&mut self, pt: &Point) {
            match *self {
                Shape::Rect {
                    ref mut p0,
                    ref mut p1,
                } => {
                    p0.x += pt.x;
                    p0.y += pt.y;
                    p1.x += pt.x;
                    p1.y += pt.y;
                }
                Shape::Poly { ref mut pts } => {
                    for p in pts.iter_mut() {
                        p.x += pt.x;
                        p.y += pt.y;
                    }
                }
            }
        }
    }
    /// # Gds21 Converter
    ///
    /// The sole valid top-level entity for [gds21] conversion is always a [Library].
    ///
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GdsConverter {
        pub lib: Library,
    }
    impl GdsConverter {
        pub fn convert(lib: Library) -> LayoutResult<gds21::GdsLibrary> {
            Self { lib }.convert_all()
        }
        fn convert_all(self) -> LayoutResult<gds21::GdsLibrary> {
            if self.lib.libs.len() > 0 {
                return Err(LayoutError::msg("No nested libraries to GDS (yet)"));
            }
            // Create a new Gds Library
            let mut lib = gds21::GdsLibrary::new(&self.lib.name);
            // Set its distance units
            lib.units = match self.lib.units {
                Unit::Nano => gds21::GdsUnits::new(1e-3, 1e-9),
                Unit::Micro => gds21::GdsUnits::new(1e-3, 1e-6),
            };
            // And convert each of our `cells` into its `structs`
            lib.structs = self
                .lib
                .cells
                .iter()
                .map(|c| self.convert_cell(c))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(lib)
        }
        /// Convert a [Cell] to a [gds21::GdsStruct] cell-definition
        fn convert_cell(&self, cell: &Cell) -> LayoutResult<gds21::GdsStruct> {
            let mut elems = Vec::new();
            for inst in cell.insts.iter() {
                elems.push(self.convert_instance(inst).into());
            }
            for arr in cell.arrays.iter() {
                elems.push(self.convert_array(arr).into());
            }
            for elem in cell.elems.iter() {
                for gdselem in self.convert_element(elem)?.into_iter() {
                    elems.push(gdselem);
                }
            }
            let mut s = gds21::GdsStruct::new(&cell.name);
            s.elems = elems;
            Ok(s)
        }
        /// Convert an [Instance] to a GDS instance, AKA [gds21::GdsStructRef]
        fn convert_instance(&self, inst: &Instance) -> gds21::GdsStructRef {
            gds21::GdsStructRef {
                name: inst.cell_name.clone(),
                xy: vec![inst.p0.x as i32, inst.p0.y as i32],
                strans: None, //FIXME!
                elflags: None,
                plex: None,
            }
        }
        /// Convert an [Element] into one or more [gds21::GdsElement]
        ///
        /// Our [Element]s often correspond to more than one GDSII element,
        /// notably in the case in which a polygon is annotated with a net-name.
        /// Here, the net-name is an attribute of the polygon [Element].
        /// In GDSII, text is "free floating" as a separate element.
        ///
        /// GDS shapes are flattened vectors of (x,y) coordinates,
        /// and include an explicit repetition of their origin for closure.
        /// So an N-sided polygon is described by a 2*(N+1)-entry vector.
        ///
        pub fn convert_element(&self, elem: &Element) -> LayoutResult<Vec<gds21::GdsElement>> {
            let datatype = elem
                .layer
                .drawing
                .ok_or(LayoutError::msg("Drawing Layer Not Defined"))?;
            let xy = match &elem.inner {
                Shape::Rect { p0, p1 } => {
                    let x0 = p0.x as i32;
                    let y0 = p0.y as i32;
                    let x1 = p1.x as i32;
                    let y1 = p1.y as i32;
                    vec![x0, y0, x1, y0, x1, y1, x0, y1, x0, y0]
                }
                Shape::Poly { pts } => {
                    // Flatten our points-vec, converting to 32-bit along the way
                    let mut xy = Vec::new();
                    for p in pts.iter() {
                        xy.push(p.x as i32);
                        xy.push(p.y as i32);
                    }
                    // Add the origin a second time, to "close" the polygon
                    xy.push(pts[0].x as i32);
                    xy.push(pts[0].y as i32);
                    xy
                }
            };
            // Initialize our vector of elements with the shape
            let mut gds_elems = vec![gds21::GdsBoundary {
                layer: elem.layer.layernum,
                datatype,
                xy,
                ..Default::default()
            }
            .into()];
            // If there's an assigned net, create a corresponding text-element
            if let Some(name) = &elem.net {
                let texttype = elem
                    .layer
                    .text
                    .ok_or(LayoutError::msg("Text Layer Not Defined"))?;

                // Text is placed in the shape's (at least rough) center
                let loc = elem.inner.center();
                // Rotate that text 90 degrees for mostly-vertical shapes
                let strans = match elem.inner.orientation() {
                    Dir::Horiz => None,
                    Dir::Vert => Some(gds21::GdsStrans {
                        angle: Some(90.0),
                        ..Default::default()
                    }),
                };
                gds_elems.push(
                    gds21::GdsTextElem {
                        string: name.into(),
                        layer: elem.layer.layernum,
                        texttype,
                        xy: vec![loc.x as i32, loc.y as i32],
                        strans,
                        ..Default::default()
                    }
                    .into(),
                )
            }
            Ok(gds_elems)
        }
        /// Convert an [InstArray] to GDS-format [gds21::GdsArrayRef]
        ///
        /// GDS requires three "points" to define an array,
        /// Essentially at its origin and opposite edges
        pub fn convert_array(&self, arr: &InstArray) -> gds21::GdsArrayRef {
            let x0 = arr.p0.x as i32;
            let y0 = arr.p0.y as i32;
            let x1 = x0 + (arr.xpitch * arr.cols + 1) as i32;
            let y1 = y0 + (arr.ypitch * arr.rows + 1) as i32;
            gds21::GdsArrayRef {
                name: arr.cell_name.clone(),
                xy: vec![x0, y0, x1, y0, x0, y1],
                rows: arr.rows as i16,
                cols: arr.cols as i16,
                strans: None, //FIXME!
                elflags: None,
                plex: None,
            }
        }
    }
    impl From<gds21::GdsError> for LayoutError {
        fn from(_e: gds21::GdsError) -> Self {
            LayoutError::Tbd
        }
    }
}
/// # Converter from [Library] and constituent elements to [raw::Library]
pub struct RawConverter {
    pub lib: Library,
}
impl RawConverter {
    /// Convert [Library] `lib` to a [raw::Library]
    /// Consumes `lib` in the process
    pub fn convert(lib: Library) -> Result<raw::Library, LayoutError> {
        Self { lib }.convert_all()
    }
    /// Convert everything in our [Library]
    fn convert_all(self) -> LayoutResult<raw::Library> {
        let mut lib = raw::Library::new(&self.lib.name, self.lib.stack.units);
        // Collect up unit-cells on each layer
        for layer in self.lib.stack.layers.iter() {
            let unit = self.convert_layer_unit(layer)?;
            lib.cells.push(unit);
        }
        // Convert each defined [Cell] to a [raw::Cell]
        for (_id, cell) in self.lib.cells.iter() {
            lib.cells.push(self.convert_cell(cell)?);
        }
        // And convert each (un-implemented) Abstract as a boundary
        for (_id, abs) in self.lib.abstracts.iter() {
            // FIXME: temporarily checking whether the same name is already defined
            for (_id, cell) in self.lib.cells.iter() {
                if abs.name == cell.name {
                    continue;
                }
            }
            lib.cells.push(self.convert_abstract(abs)?);
        }
        Ok(lib)
    }
    /// Convert to a raw layout cell
    fn convert_cell(&self, cell: &Cell) -> Result<raw::Cell, LayoutError> {
        let lib: &Library = &self.lib;
        println!("TO RAW CELL {:?}", cell.name);

        if cell.outline.x.len() > 1 {
            return Err(LayoutError::Message(
                "Non-rectangular outline; not supported (yet)".into(),
            ));
        };
        let mut elems: Vec<raw::Element> = Vec::new();

        /// A short-lived set of references to an [Instance] and its cell-definition
        #[derive(Debug, Clone)]
        struct TempInstance<'a> {
            inst: &'a Instance,
            def: &'a (dyn HasOutline + 'static),
        }
        // Create one of these for each of our instances
        let temp_instances: Vec<TempInstance> = cell
            .instances
            .iter()
            .map(|inst| {
                match inst.cell {
                    CellRef::Cell(c) => {
                        let def = lib.cells.get(c).ok_or(LayoutError::Tbd).unwrap();
                        TempInstance { inst, def }
                    }
                    CellRef::Abstract(c) => {
                        let def = lib.abstracts.get(c).ok_or(LayoutError::Tbd).unwrap();
                        TempInstance { inst, def }
                    }
                    _ => panic!("FIXME!"),
                    // _ => return Err(LayoutError::Tbd),
                }
            })
            .collect();

        // Collect our assignments up by layer
        let mut assignments_by_layer: Vec<Vec<&Assign>> = vec![vec![]; cell.top_layer()];
        let mut inverse_assignments_by_layer: Vec<Vec<&Assign>> = vec![vec![]; cell.top_layer()];
        for assn in cell.assignments.iter() {
            assignments_by_layer[assn.at.layer].push(&assn);
            let other = match assn.at.relz {
                RelZ::Above => assn.at.layer + 1,
                RelZ::Below => assn.at.layer - 1,
            };
            inverse_assignments_by_layer[other].push(&assn);
        }

        // Iterate over tracks, chopping them at instances and cuts
        for layernum in 0..cell.top_layer {
            let layer = &lib.stack.layers[layernum];
            println!("LAYER: {:?}", layer.index);

            // Sort out which of our [Instance]s come up to this layer
            let layer_instances: Vec<&TempInstance> = temp_instances
                .iter()
                .filter(|i| i.def.top_layer() >= layer.index)
                .collect();
            println!("LAYER_INSTS: {:?}", layer_instances);

            // Sort out which direction we're working across
            let (m, n, pitch) = match layer.dir {
                Dir::Horiz => (cell.outline.y[0], cell.outline.x[0], lib.stack.xpitch),
                Dir::Vert => (cell.outline.x[0], cell.outline.y[0], lib.stack.ypitch),
            };
            let pitch = pitch as isize;

            for rown in 0..m {
                let rown = rown as isize;
                println!("ROWN: {:?}", rown);
                // For each row, decide which instances intersect
                let intersecting_instances: Vec<&TempInstance> = layer_instances
                    .iter()
                    .filter(|i| {
                        i.inst.p0.coord(layer.dir.other()) <= rown
                            && i.inst.p0.coord(layer.dir.other())
                                + i.def.outline().max(layer.dir.other()) as isize
                                > rown
                    })
                    .map(|i| i.clone())
                    .collect();
                println!("INTERSECTING_INSTS: {:?}", intersecting_instances);
                // Convert these into blockage-areas for the tracks
                let blockages: Vec<(usize, usize)> = intersecting_instances
                    .iter()
                    .map(|i| {
                        (
                            i.inst.p0.coord(layer.dir) as usize,
                            i.inst.p0.coord(layer.dir) as usize + i.def.outline().max(layer.dir),
                        )
                    })
                    .collect();

                let mut track_period = layer.to_track_period(pitch as usize * n);
                for (n1, n2) in blockages.iter() {
                    track_period.cut(*n1 * pitch as usize, *n2 * pitch as usize)?;
                }
                // Handle Net Assignments
                // First filter down to the ones in our row/col
                let nsig = track_period.signals.len();
                let relevant_track_nums = (rown * nsig as isize, (rown + 1) * nsig as isize);
                let relevant_assignments: &Vec<&Assign> = &assignments_by_layer[layernum]
                    .iter()
                    .filter(|assn| {
                        assn.at.track >= relevant_track_nums.0 as usize
                            && assn.at.track < relevant_track_nums.1 as usize
                    })
                    .copied()
                    .collect();
                println!("RELEVANT_ASSIGNMENTS: {:?}", relevant_assignments);
                for assn in relevant_assignments.iter() {
                    // Grab a (mutable) reference to the assigned track
                    let track = &mut track_period.signals[assn.at.track & nsig];

                    // Figure out the off-axis coordinate
                    let other_layer: &Layer = match assn.at.relz {
                        RelZ::Above => &lib.stack.layers[layernum + 1],
                        RelZ::Below => &lib.stack.layers[layernum - 1],
                    };
                    let dist = other_layer.signal_track_center(assn.at.at);
                    // Find the segment corresponding to the off-axis coordinate
                    let mut segment = track
                        .segment_at(dist)
                        .ok_or(LayoutError::msg("COULDNT FIND SEGMENT"))?;
                    // Assign both track-segments to the net
                    segment.net = Some(assn.net.clone());
                    // FIXME: Insert a corresponding via
                }
                // And assignments for which this is the secondary layer
                for assn in inverse_assignments_by_layer.iter() {
                    // unimplemented!("???");
                }
                // Convert all TrackSegments to raw Elements
                let shift = Point::offset(rown * pitch, layer.dir.other());
                let mut push_track = |t: &Track| {
                    for mut e in self.convert_track(t).unwrap().into_iter() {
                        e.inner.shift(&shift);
                        elems.push(e);
                    }
                };
                for t in track_period.rails.iter() {
                    push_track(t);
                }
                for t in track_period.signals.iter() {
                    push_track(t);
                }
                println!("ELEMS: {:?}", elems);
            }
        }
        // FIXME: handle cuts!
        // for cut in cell.cuts.iter() {
        //     unimplemented!("???");
        //     // Split the track into segments
        // }

        // Convert our [Outline] into a polygon
        elems.push(self.convert_outline(&cell.outline)?);
        // Convert our [Instance]s dimensions
        // Note instances are of the same type, but use [Points] of different units.
        let scale = (lib.stack.xpitch as isize, lib.stack.ypitch as isize);
        let insts = cell
            .instances
            .iter()
            .map(|inst| {
                // Scale the location of each instance by our pitches
                let mut i = inst.clone();
                i.p0 = inst.p0.scale(scale.0, scale.1);
                i
            })
            .collect();
        // Aaaand create & return our new [raw::Cell]
        Ok(raw::Cell {
            name: cell.name.clone(),
            insts,
            arrays: Vec::new(),
            elems,
        })
    }

    /// Convert to a [raw::Cell], just including an Outline
    /// FIXME: also include the pins!
    pub fn convert_abstract(&self, abs: &abstrakt::Abstract) -> Result<raw::Cell, LayoutError> {
        // Create our [Outline]s boundary
        let outline = self.convert_outline(&abs.outline)?;
        // And return a new [raw::Cell]
        Ok(raw::Cell {
            name: abs.name.clone(),
            insts: Vec::new(),
            arrays: Vec::new(),
            elems: vec![outline],
        })
    }
    /// Convert to a [raw::Element] polygon
    pub fn convert_outline(&self, outline: &Outline) -> Result<raw::Element, LayoutError> {
        // Doing so requires our [Stack] specify a `boundary_layer`. If not, fail.
        let layer = (self.lib.stack.boundary_layer)
            .as_ref()
            .ok_or(LayoutError::msg(
                "Cannot Convert Abstract to Raw without Boundary Layer",
            ))?;
        // Create an array of Outline-Points
        let pts = outline.points();
        // Scale them to our pitches
        let pitch = (
            self.lib.stack.xpitch as isize,
            self.lib.stack.ypitch as isize,
        );
        let pts = pts.iter().map(|p| p.scale(pitch.0, pitch.1)).collect();
        // Create the [raw::Element]
        Ok(raw::Element {
            net: None,
            layer: layer.clone(), // FIXME: stop cloning
            inner: raw::Shape::Poly { pts },
        })
    }
    /// Convert a [Track]-full of [TrackSegment]s to a vector of [raw::Element] rectangles
    fn convert_track(&self, track: &Track) -> LayoutResult<Vec<raw::Element>> {
        let layer = track
            .layer
            .raw
            .as_ref()
            .ok_or(LayoutError::msg("Raw-Layout Layer Not Defined"))?;

        let elems = track
            .segments
            .iter()
            .map(|seg| {
                match track.dir {
                    Dir::Horiz => raw::Element {
                        net: seg.net.clone(),
                        layer: layer.clone(), // FIXME: dont really wanna clone here
                        inner: raw::Shape::Rect {
                            p0: Point {
                                x: (seg.start) as isize,
                                y: (track.start as isize),
                            },
                            p1: Point {
                                x: (seg.stop) as isize,
                                y: (track.start + track.width as isize) as isize,
                            },
                        },
                    },
                    Dir::Vert => raw::Element {
                        net: seg.net.clone(),
                        layer: layer.clone(), // FIXME: dont really wanna clone here
                        inner: raw::Shape::Rect {
                            p0: Point {
                                x: (track.start as isize),
                                y: (seg.start) as isize,
                            },
                            p1: Point {
                                x: (track.start + track.width as isize) as isize,
                                y: (seg.stop) as isize,
                            },
                        },
                    },
                }
            })
            .collect();
        Ok(elems)
    }
    /// Create a raw-cell covering a single unit of `layer`
    pub fn convert_layer_unit(&self, layer: &Layer) -> LayoutResult<raw::Cell> {
        let pitch = match layer.dir {
            Dir::Horiz => self.lib.stack.xpitch,
            Dir::Vert => self.lib.stack.ypitch,
        };
        let mut elems: Vec<raw::Element> = Vec::new();
        // FIXME: probably get away from this Layer::tracks method. Everything else has.
        for track in layer.tracks().iter_mut() {
            track.segments = vec![TrackSegment {
                net: None, // FIXME!
                start: 0,
                stop: pitch,
            }];
            // Convert into [raw::Element] rectangles.
            // This vector always has just one element, but is easier to iterate over (once).
            for e in self.convert_track(&track)?.into_iter() {
                elems.push(e);
            }
        }
        Ok(raw::Cell {
            name: format!("{}::unit", layer.name.clone()),
            insts: Vec::new(),
            arrays: Vec::new(),
            elems,
        })
    }
}
/// # Abstract Layout Module
///
/// Abstract layouts describe a block's outline and interface,
/// without exposing implementation details.
/// Cells primarily comprise their outlines and pins.
/// Outlines follow the same "Tetris-Shapes" as (OtherNameTbd) layout cells,
/// including the requirements for a uniform z-axis.
/// Internal layers are "fully blocked", in that parent layouts may not route through them.
/// In legacy layout systems this would be akin to including blockages of the same shape as [Outline] on each layer.
///
/// Sadly the english-spelled name "abstract" is reserved as a potential [future Rust keyword](https://doc.rust-lang.org/reference/keywords.html#reserved-keywords).
/// Hence the misspelling.
///
pub mod abstrakt {
    use super::*;
    // FIXME: also need a raw::Abstract, for more-arbitrary-shaped abstract layouts

    /// Abstract-Layout
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Abstract {
        /// Cell Name
        pub name: String,
        /// Outline in "Tetris-Shapes"
        pub outline: Outline,
        /// Top Metal Layer
        pub top_layer: usize,
        /// Ports
        pub ports: Vec<Port>,
    }
    /// Abstract-Layout Port
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Port {
        /// Port/ Signal Name
        pub name: String,
        /// Physical Info
        pub kind: PortKind,
    }
    /// Abstract-Layout Port Inner Detail
    ///
    /// All location and "geometric" information per Port is stored here,
    /// among a few enumerated variants.
    ///
    /// Ports may either connect on x/y edges, or on the top (in the z-axis) layer.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum PortKind {
        /// Ports which connect on x/y outline edges
        Edge {
            layer: usize,
            track: usize,
            side: Side,
        },
        /// Ports which are internal to the cell outline,
        /// but connect from above in the z-stack.
        /// These can be assigned at several locations across their track,
        /// and are presumed to be internally-connected between such locations.
        Zlocs {
            /// Locations
            locs: Vec<TopLoc>,
        },
        /// Ports which occupy an entire top-level track from edge to edge
        Zfull { track: usize },
        // FIXME:
        // * Sort out cases for "both", i.e. pins on the top-level which also go to X/Y edges
        // * Primitives may need a different kinda `cross`
    }
    /// A location (track intersection) on our top z-axis layer
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TopLoc {
        /// Track Index
        track: usize,
        /// Intersecting Track Index
        at: usize,
        /// Whether `at` refers to the track-indices above or below
        relz: RelZ,
    }
    /// X/Y Side Enumeration
    /// Note the requirements on [Outline] shapes ensure each track has a unique left/right or top/bottom pair of edges.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Side {
        Left,
        Right,
        Top,
        Bottom,
    }
}
/// Interfaces Module,
/// Describing Cells in terms of their IO Interfaces
pub mod interface {
    use serde::{Deserialize, Serialize};
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Port {
        /// Port Name
        pub name: String,
        /// Port Type & Content
        pub kind: PortKind,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum PortKind {
        /// Flat Scalar Port, e.g. `clk`
        Scalar,
        /// Array-Based Port, e.g. `data[31:0]`
        Array { width: usize },
        /// Instance of a Hierarchical Bundle
        Bundle { bundle_name: String },
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Bundle {
        pub name: String,
        pub ports: Vec<Port>,
    }
}
/// # Cell View Enumeration
/// All of the ways in which a Cell is represented
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CellView {
    Interface(interface::Bundle),
    Abstract(abstrakt::Abstract),
    Layout(Cell),
    RawLayout(raw::Cell),
}
/// Collection of the Views describing a Cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellViews {
    name: String,
    views: SlotMap<CellViewKey, CellView>,
}

///
/// # Layout Error Enumeration
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutError {
    /// Uncategorized Error with Message
    Message(String),
    /// Error Exporting to Foreign Format
    Export,
    /// Everything to be categorized
    Tbd,
}
impl LayoutError {
    /// Create a [LayoutError::Message] from anything String-convertible
    fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}
/// # Cell Reference Enumeration
/// Used for enumerating the different types of things an [Instance] may refer to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CellRef {
    Cell(CellKey),
    Abstract(AbstractKey),
    Name(String),
}
/// Trait for accessing three-dimensional [Outline] data from several views of Layouts
trait HasOutline: Debug {
    /// Retrieve a reference to the x-y [Outline]
    fn outline(&self) -> &Outline;
    /// Retrieve the top z-axis layer
    fn top_layer(&self) -> usize;
}
impl HasOutline for Cell {
    fn outline(&self) -> &Outline {
        &self.outline
    }
    fn top_layer(&self) -> usize {
        self.top_layer
    }
}
impl HasOutline for abstrakt::Abstract {
    fn outline(&self) -> &Outline {
        &self.outline
    }
    fn top_layer(&self) -> usize {
        self.top_layer
    }
}
/// Placeholder for Elements to be implemented
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tbd;

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a [Stack] used by a number of tests
    fn stack() -> Stack {
        use raw::LayerSpec;

        Stack {
            units: Unit::Nano,
            xpitch: 6600,
            ypitch: 6600,
            boundary_layer: Some(raw::DataTypeMap {
                layernum: 236,
                drawing: Some(0),
                text: None,
                other: HashMap::new(),
            }),
            layers: vec![
                Layer {
                    index: 1,
                    name: "met1".into(),
                    entries: vec![
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                    ],
                    dir: Dir::Horiz,
                    offset: -245,
                    cutsize: 50,
                    stream_layer: Some(LayerSpec::new(68, 20)),
                    raw: Some(raw::DataTypeMap {
                        layernum: 68,
                        drawing: Some(20),
                        text: Some(5),
                        other: HashMap::new(),
                    }),
                },
                Layer {
                    index: 2,
                    name: "met2".into(),
                    entries: vec![
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                    ],
                    dir: Dir::Vert,
                    cutsize: 50,
                    offset: -245,
                    stream_layer: Some(LayerSpec::new(69, 20)),

                    raw: Some(raw::DataTypeMap {
                        layernum: 69,
                        drawing: Some(20),
                        text: Some(5),
                        other: HashMap::new(),
                    }),
                },
                Layer {
                    index: 3,
                    name: "met3".into(),
                    entries: vec![
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                    ],
                    dir: Dir::Horiz,
                    cutsize: 50,
                    offset: -245,
                    stream_layer: Some(LayerSpec::new(70, 20)),

                    raw: Some(raw::DataTypeMap {
                        layernum: 70,
                        drawing: Some(20),
                        text: Some(5),
                        other: HashMap::new(),
                    }),
                },
                Layer {
                    index: 4,
                    name: "met4".into(),
                    entries: vec![
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                        TrackSpec::pat(vec![TrackEntry::gap(230), TrackEntry::sig(140)], 7),
                        TrackSpec::gap(230),
                        TrackSpec::rail(490),
                    ],
                    dir: Dir::Vert,
                    cutsize: 50,
                    offset: -245,
                    stream_layer: Some(LayerSpec::new(71, 20)),

                    raw: Some(raw::DataTypeMap {
                        layernum: 71,
                        drawing: Some(20),
                        text: Some(5),
                        other: HashMap::new(),
                    }),
                },
            ],
            vias: vec![
                ViaLayer {
                    index: 0,
                    name: "mcon".into(),
                    between: (0, 1),
                    size: Point::new(140, 140),
                    stream_layer: Some(LayerSpec::new(67, 44)),
                },
                ViaLayer {
                    index: 1,
                    name: "via1".into(),
                    between: (1, 2),
                    size: Point::new(140, 140),
                    stream_layer: Some(LayerSpec::new(68, 44)),
                },
                ViaLayer {
                    index: 2,
                    name: "via2".into(),
                    between: (2, 3),
                    size: Point::new(140, 140),
                    stream_layer: Some(LayerSpec::new(69, 44)),
                },
            ],
        }
    }

    /// Create a cell
    #[test]
    fn create_cell() -> Result<(), LayoutError> {
        Cell {
            name: "HereGoes".into(),
            top_layer: 3,
            outline: Outline::rect(5, 5)?,
            instances: Vec::new(),
            assignments: vec![Assign {
                net: "clk".into(),
                at: TrackIntersection {
                    layer: 1,
                    track: 0,
                    at: 1,
                    relz: RelZ::Above,
                },
            }],
            cuts: Vec::new(),
        };
        Ok(())
    }
    /// Create a library
    #[test]
    fn create_lib() -> Result<(), LayoutError> {
        let mut lib = Library::new("HereGoesLib", stack());

        let c = lib.cells.insert(Cell {
            name: "HereGoes".into(),
            top_layer: 3,
            outline: Outline::rect(5, 5)?,
            instances: Vec::new(),
            assignments: vec![Assign {
                net: "clk".into(),
                at: TrackIntersection {
                    layer: 1,
                    track: 0,
                    at: 1,
                    relz: RelZ::Above,
                },
            }],
            cuts: Vec::new(),
        });
        exports(lib)
    }
    /// Create a cell with instances
    #[test]
    fn create_lib2() -> Result<(), LayoutError> {
        let mut lib = Library::new("InstLib", stack());

        let c2 = lib.cells.insert(Cell {
            name: "IsInst".into(),
            top_layer: 2,
            outline: Outline::rect(1, 1)?,
            instances: vec![],
            assignments: vec![],
            cuts: Vec::new(),
        });

        let c = lib.cells.insert(Cell {
            name: "HasInst".into(),
            top_layer: 4,
            outline: Outline::rect(5, 11)?,
            instances: vec![Instance {
                inst_name: "inst1".into(),
                cell_name: "IsInst".into(),
                cell: CellRef::Cell(c2),
                p0: Point::new(1, 2),
                reflect: false,
                angle: None,
            }],
            assignments: vec![Assign {
                net: "clk".into(),
                at: TrackIntersection {
                    layer: 1,
                    track: 0,
                    at: 1,
                    relz: RelZ::Above,
                },
            }],
            cuts: Vec::new(),
        });
        exports(lib)
    }

    /// Create an abstract layout, with its variety of supported port types
    #[test]
    fn create_abstract() -> Result<(), LayoutError> {
        let outline = Outline::rect(11, 11)?;
        let ports = vec![
            abstrakt::Port {
                name: "edge_bot".into(),
                kind: abstrakt::PortKind::Edge {
                    layer: 2,
                    track: 2,
                    side: abstrakt::Side::Bottom,
                },
            },
            abstrakt::Port {
                name: "edge_top".into(),
                kind: abstrakt::PortKind::Edge {
                    layer: 2,
                    track: 4,
                    side: abstrakt::Side::Top,
                },
            },
            abstrakt::Port {
                name: "edge_left".into(),
                kind: abstrakt::PortKind::Edge {
                    layer: 1,
                    track: 1,
                    side: abstrakt::Side::Left,
                },
            },
            abstrakt::Port {
                name: "edge_right".into(),
                kind: abstrakt::PortKind::Edge {
                    layer: 1,
                    track: 5,
                    side: abstrakt::Side::Right,
                },
            },
            abstrakt::Port {
                name: "zfull".into(),
                kind: abstrakt::PortKind::Zfull { track: 3 },
            },
            // abstrakt::Port {
            //     name: "zlocs".into(),
            //     kind: abstrakt::PortKind::Zlocs {
            //         locs: vec![Assign {}],
            //     },
            // },
        ];
        abstrakt::Abstract {
            name: "abstrack".into(),
            outline,
            top_layer: 3,
            ports,
        };
        Ok(())
    }

    /// Create a cell with abstract instances
    #[test]
    fn create_lib3() -> Result<(), LayoutError> {
        let mut lib = Library::new("InstLib", stack());

        let c2 = lib.abstracts.insert(abstrakt::Abstract {
            name: "IsAbstrakt".into(),
            top_layer: 2,
            outline: Outline::rect(1, 1)?,
            ports: Vec::new(),
        });

        let c = lib.cells.insert(Cell {
            name: "HasAbstrakts".into(),
            top_layer: 3,
            outline: Outline::rect(5, 5)?,
            instances: vec![
                Instance {
                    inst_name: "inst1".into(),
                    cell_name: "IsAbstrakt".into(),
                    cell: CellRef::Abstract(c2),
                    p0: Point::new(0, 0),
                    reflect: false,
                    angle: None,
                },
                Instance {
                    inst_name: "inst2".into(),
                    cell_name: "IsAbstrakt".into(),
                    cell: CellRef::Abstract(c2),
                    p0: Point::new(2, 2),
                    reflect: false,
                    angle: None,
                },
                Instance {
                    inst_name: "inst4".into(),
                    cell_name: "IsAbstrakt".into(),
                    cell: CellRef::Abstract(c2),
                    p0: Point::new(4, 4),
                    reflect: false,
                    angle: None,
                },
            ],
            assignments: vec![Assign {
                net: "clk".into(),
                at: TrackIntersection {
                    layer: 1,
                    track: 11,
                    at: 11,
                    relz: RelZ::Above,
                },
            }],
            cuts: Vec::new(),
        });
        exports(lib)
    }
    /// Export [Library] `lib` in several formats
    fn exports(lib: Library) -> LayoutResult<()> {
        save_yaml(&lib, &resource(&format!("{}.yaml", &lib.name)))?;
        let raw = RawConverter::convert(lib)?;
        save_yaml(&raw, &resource(&format!("{}.raw.yaml", &raw.name)))?;
        let gds = raw.to_gds()?;
        save_yaml(&gds, &resource(&format!("{}.gds.yaml", &gds.name)))?;
        gds.save(&resource(&format!("{}.gds", &gds.name)))?;
        Ok(())
    }
    #[allow(unused_imports)]
    use std::io::prelude::*;
    #[test]
    fn stack_to_yaml() -> LayoutResult<()> {
        save_yaml(&stack(), &resource("stack.yaml"))
    }
    /// Grab the full path of resource-file `fname`
    fn resource(fname: &str) -> String {
        format!("{}/resources/{}", env!("CARGO_MANIFEST_DIR"), fname)
    }
    /// Save any [Serialize]-able type to yaml-format file `fname`
    fn save_yaml(data: &impl Serialize, fname: &str) -> LayoutResult<()> {
        use std::fs::File;
        use std::io::BufWriter;
        let mut file = BufWriter::new(File::create(fname).unwrap());
        let yaml = serde_yaml::to_string(data).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();
        file.flush().unwrap();
        Ok(())
    }
}
