import AppKit

guard CommandLine.arguments.count == 2 else { fatalError("Usage: make-icon.swift ICONSET") }
let destination = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)

func color(_ hex: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 255) / 255,
            green: CGFloat((hex >> 8) & 255) / 255,
            blue: CGFloat(hex & 255) / 255, alpha: 1)
}

for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
                                  bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                  isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        let transform = NSAffineTransform()
        transform.scale(by: CGFloat(pixels) / 1024)
        transform.concat()
        color(0x20272d).setFill()
        NSBezierPath(roundedRect: NSRect(x: 64, y: 64, width: 896, height: 896), xRadius: 200, yRadius: 200).fill()
        let left = NSBezierPath()
        left.move(to: NSPoint(x: 220, y: 740))
        left.curve(to: NSPoint(x: 500, y: 702), controlPoint1: NSPoint(x: 320, y: 760), controlPoint2: NSPoint(x: 410, y: 750))
        left.line(to: NSPoint(x: 500, y: 270))
        left.curve(to: NSPoint(x: 220, y: 308), controlPoint1: NSPoint(x: 410, y: 318), controlPoint2: NSPoint(x: 320, y: 328))
        left.close()
        color(0xf3f0e8).setFill()
        left.fill()
        let right = NSBezierPath()
        right.move(to: NSPoint(x: 524, y: 702))
        right.curve(to: NSPoint(x: 804, y: 740), controlPoint1: NSPoint(x: 614, y: 750), controlPoint2: NSPoint(x: 704, y: 760))
        right.line(to: NSPoint(x: 804, y: 308))
        right.curve(to: NSPoint(x: 524, y: 270), controlPoint1: NSPoint(x: 704, y: 328), controlPoint2: NSPoint(x: 614, y: 318))
        right.close()
        color(0x69d6c5).setFill()
        right.fill()
        NSGraphicsContext.restoreGraphicsState()
        let suffix = scale == 2 ? "@2x" : ""
        let output = destination.appendingPathComponent("icon_\(size)x\(size)\(suffix).png")
        try rep.representation(using: .png, properties: [:])!.write(to: output)
    }
}
