#!/usr/bin/swift
// assets/disk-lens.svg → macOS iconset, assets/disk-lens.ico, assets/icon.png
// 사용법 (disk-lens 폴더에서, macOS 기본 도구만 사용):
//   swift scripts/make-icon.swift <iconset 폴더>   앱 번들용 iconset
//   swift scripts/make-icon.swift --assets         저장소에 넣어 두는 ico 와 png 를 다시 만든다
import AppKit

guard let svg = NSImage(contentsOf: URL(fileURLWithPath: "assets/disk-lens.svg")) else {
    fatalError("assets/disk-lens.svg 를 읽지 못했습니다. disk-lens 폴더에서 실행하세요.")
}

func png(_ size: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: size, height: size)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    svg.draw(in: NSRect(x: 0, y: 0, width: size, height: size))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let argument = CommandLine.arguments.dropFirst().first ?? "dist/DiskLens.iconset"
if argument == "--assets" {
    // Windows 실행 파일에 넣는 ico. Windows 에서는 SVG 를 그릴 도구가 없어 만든 결과를 저장소에 둔다.
    let sizes = [16, 20, 24, 32, 40, 48, 64, 256]
    let images = sizes.map(png)
    func le<T: FixedWidthInteger>(_ value: T) -> Data { withUnsafeBytes(of: value.littleEndian) { Data($0) } }
    var ico = le(UInt16(0)) + le(UInt16(1)) + le(UInt16(sizes.count))
    var offset = 6 + 16 * sizes.count
    for (size, image) in zip(sizes, images) {
        let side = UInt8(size == 256 ? 0 : size)
        ico += Data([side, side, 0, 0]) + le(UInt16(1)) + le(UInt16(32)) + le(UInt32(image.count)) + le(UInt32(offset))
        offset += image.count
    }
    images.forEach { ico += $0 }
    try ico.write(to: URL(fileURLWithPath: "assets/disk-lens.ico"))
    // README 와 Windows 창 아이콘
    try png(512).write(to: URL(fileURLWithPath: "assets/icon.png"))
    print("만들었습니다: assets/disk-lens.ico, assets/icon.png")
} else {
    let iconset = URL(fileURLWithPath: argument)
    try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
    for base in [16, 32, 128, 256, 512] {
        try png(base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
        try png(base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
    }
}
