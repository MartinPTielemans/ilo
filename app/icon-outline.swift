// Makes the app icon's one layer from the knot's drawing, for icon.sh. The system's icon renderer
// draws fills, not strokes or masks, and lights the edge of every shape it is given, so the cord
// and its over-passes as shapes of their own show a seam wherever they meet. So this strokes the
// drawing's centre-lines as the SVG does, cuts the under-pass gaps out of the cord as the
// drawing's mask does, and joins the over-passes on: one filled shape, no point of the drawing
// moved.
//
// The centre-lines are curves, and so is the outline: the cord's edge has no corner but where a
// gap or an over-pass ends. With `lines` its curves are written as lines, for what can fill only
// polygons (the Windows icons' script). The two smaller drawings are outlined that way too; the
// smallest has no mask, as its cord passes over nothing, so nothing is cut from it and its loops
// are holes in one piece, which the even-odd fill leaves open.
//
// usage: icon-outline <knot.svg> <layer.svg> [lines]
import CoreGraphics
import Foundation

let args = CommandLine.arguments
guard args.count == 3 || (args.count == 4 && args[3] == "lines") else {
  print("usage: icon-outline <knot.svg> <layer.svg> [lines]")
  exit(2)
}
let svg = try! String(contentsOfFile: args[1], encoding: .utf8)

struct Line {
  var path: CGPath
  var width: CGFloat
  var round: Bool
}

func lines(_ text: String) -> [Line] {
  let tag = try! NSRegularExpression(pattern: "<path d=\"([^\"]+)\"([^>]*)/>")
  let command = try! NSRegularExpression(pattern: "([A-Za-z])([^A-Za-z]*)")
  let widthOf = try! NSRegularExpression(pattern: "stroke-width=\"([\\d.]+)\"")
  let whole = text as NSString
  return tag.matches(in: text, range: NSRange(location: 0, length: whole.length)).map { found in
    let d = whole.substring(with: found.range(at: 1)) as NSString
    let rest = whole.substring(with: found.range(at: 2)) as NSString
    let path = CGMutablePath()
    // The second control point of the curve before, which S continues from.
    var before: CGPoint?
    for found in command.matches(in: d as String, range: NSRange(location: 0, length: d.length)) {
      let letter = d.substring(with: found.range(at: 1))
      let v = d.substring(with: found.range(at: 2))
        .split(whereSeparator: { $0 == " " || $0 == "," }).map { CGFloat(Double($0)!) }
      func at(_ i: Int) -> CGPoint { CGPoint(x: v[2 * i], y: v[2 * i + 1]) }
      // The drawing has these five, each with one set of numbers. Anything else would be drawn
      // wrong without a word, so a drawing that gains another command stops here.
      let takes = ["M": 2, "L": 2, "C": 6, "S": 4, "Z": 0][letter]
      precondition(takes == v.count, "a path command other than M, L, C, S, Z, or its numbers")
      switch letter {
      case "M": path.move(to: at(0))
      case "L": path.addLine(to: at(0))
      case "C": path.addCurve(to: at(2), control1: at(0), control2: at(1))
      case "S":
        // Its first control point is the one before, mirrored in the point the curve starts at:
        // the line has one direction there.
        let from = path.currentPoint
        precondition(before != nil, "an S that follows no curve")
        let mirrored = CGPoint(x: 2 * from.x - before!.x, y: 2 * from.y - before!.y)
        path.addCurve(to: at(1), control1: mirrored, control2: at(0))
      default: path.closeSubpath()
      }
      before = letter == "C" ? at(1) : letter == "S" ? at(0) : nil
    }
    let w = widthOf.firstMatch(in: rest as String, range: NSRange(location: 0, length: rest.length))!
    return Line(
      path: path, width: CGFloat(Double(rest.substring(with: w.range(at: 1)))!),
      round: rest.contains("stroke-linejoin=\"round\""))
  }
}

// The mask's strokes are the gaps; after it come the cord, then the over-passes. A drawing with no
// mask is the cord alone.
let parts = svg.components(separatedBy: "</defs>")
precondition(parts.count <= 2, "more than one mask in the drawing")
let gaps = parts.count == 2 ? lines(parts[0]) : []
let drawn = lines(parts[parts.count - 1])
precondition(
  gaps.isEmpty ? drawn.count == 1 : drawn.count > 1,
  "neither a cord with its mask and over-passes nor a cord alone")

// SVG's defaults, which the drawing leaves as they are: butt ends, a miter join limited at 4.
func outline(_ line: Line) -> CGPath {
  line.path.copy(
    strokingWithWidth: line.width, lineCap: .butt, lineJoin: line.round ? .round : .miter,
    miterLimit: 4)
}

var cut: CGPath = CGMutablePath()
for gap in gaps { cut = cut.union(outline(gap), using: .winding) }
var shape = outline(drawn[0]).normalized(using: .winding).subtracting(cut, using: .winding)
for over in drawn.dropFirst() { shape = shape.union(outline(over), using: .winding) }

// The tile as drawn, 232 across, has the knot at translate(34.8 32.8) scale(1.3533), which puts
// the knot's weight on the tile's centre with more room above it than below. The icon keeps that
// and has the knot 1.35 times as large about the centre: from there its three holes stay open at
// 16 pt.
let canvas = 1024.0
let inTile = 1.35
var place = CGAffineTransform.identity
  .translatedBy(x: canvas / 2, y: canvas / 2).scaledBy(x: inTile, y: inTile)
  .translatedBy(x: -canvas / 2, y: -canvas / 2)
  .scaledBy(x: canvas / 232, y: canvas / 232).translatedBy(x: 34.8, y: 32.8)
  .scaledBy(x: 1.3533, y: 1.3533)
// As lines, the outline stays within a hundredth of a point of the 1024 from its curves: under
// what any size the icon is drawn at can show.
let whole = shape.copy(using: &place)!
let placed = args.count == 4 ? whole.flattened(threshold: 0.01) : whole

var d = ""
var pieces = 0
var last = ""
func n(_ v: CGFloat) -> String { String(format: "%.3f", v) }
placed.applyWithBlock { element in
  let p = element.pointee.points
  func at(_ i: Int) -> String { "\(n(p[i].x)) \(n(p[i].y))" }
  switch element.pointee.type {
  case .moveToPoint:
    pieces += 1
    last = at(0)
    d += "M" + last
  case .addLineToPoint:
    // Where two of the shapes met, the union can leave their point twice in a row: an edge with
    // no length, the one trace of a join in the outline. It is written once.
    if at(0) != last { d += "L" + at(0) }
    last = at(0)
  case .addQuadCurveToPoint:
    last = at(1)
    d += "Q\(at(0)) \(last)"
  case .addCurveToPoint:
    last = at(2)
    d += "C\(at(0)) \(at(1)) \(last)"
  case .closeSubpath: d += "Z"
  @unknown default: break
  }
}
// The fill is the layer's in icon.json; black here is only what an SVG needs to be seen.
let out =
  "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1024\" height=\"1024\" viewBox=\"0 0 1024 1024\">"
  + "<path fill=\"#000000\" fill-rule=\"evenodd\" d=\"\(d)\"/></svg>\n"
try! out.write(toFile: args[2], atomically: true, encoding: .utf8)

let box = placed.boundingBoxOfPath
print(
  "icon-outline: \(pieces) closed pieces, ink x \(n(box.minX)) to \(n(box.maxX)), "
    + "y \(n(box.minY)) to \(n(box.maxY)) of \(Int(canvas))")
