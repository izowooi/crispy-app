import AppKit
import ImageIO
import UniformTypeIdentifiers

private enum ReaderAppearance {
    private static let values: [String: Any] = {
        guard let url = Bundle.main.url(forResource: "ReaderTheme", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let values = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return values
    }()
    static func metric(_ key: String, _ fallback: CGFloat) -> CGFloat {
        (values[key] as? NSNumber).map { CGFloat($0.doubleValue) } ?? fallback
    }
    static func color(_ key: String, _ fallback: NSColor) -> NSColor {
        guard let text = values[key] as? String, let rgb = UInt32(text.replacingOccurrences(of: "#", with: ""), radix: 16) else { return fallback }
        return NSColor(calibratedRed: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: 1)
    }
    static func points(_ key: String, _ fallback: [CGPoint]) -> [CGPoint] {
        guard let pairs = values[key] as? [[NSNumber]], pairs.count >= 3, pairs.allSatisfy({ $0.count == 2 }) else { return fallback }
        return pairs.map { CGPoint(x: CGFloat($0[0].doubleValue), y: CGFloat($0[1].doubleValue)) }
    }
}

private let launchTime = DispatchTime.now().uptimeNanoseconds
private let testing = ProcessInfo.processInfo.environment["SUA_VIEW_TESTING"] == "1"

private func isLandscape(_ size: CGSize) -> Bool {
    sua_page_is_landscape(Double(size.width), Double(size.height)) != 0
}

private func canPair(_ index: Int, end: Int, spread: Bool, cover: Bool, firstLandscape: Bool, secondLandscape: Bool = false) -> Bool {
    sua_can_pair(index, end, spread ? 1 : 0, cover ? 1 : 0, firstLandscape ? 1 : 0, secondLandscape ? 1 : 0) != 0
}

private func metric(_ event: String, _ fields: [String: Any] = [:]) {
    guard let path = ProcessInfo.processInfo.environment["SUA_VIEW_METRICS"] else { return }
    var record = fields
    record["event"] = event
    record["elapsed_ms"] = Double(DispatchTime.now().uptimeNanoseconds - launchTime) / 1_000_000
    guard var data = try? JSONSerialization.data(withJSONObject: record, options: [.sortedKeys]) else { return }
    data.append(10)
    if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
    if let file = FileHandle(forWritingAtPath: path) { file.seekToEndOfFile(); file.write(data); try? file.close() }
}

private struct PageImage {
    let index: Int
    let name: String
    let image: CGImage
    let originalSize: CGSize
    let pixelLimit: Int
    var cost: Int { image.bytesPerRow * image.height }
    var landscape: Bool { isLandscape(originalSize) }
}

private struct DecodeTarget {
    let viewport: CGSize
    let scale: CGFloat
    let zoom: CGFloat
    let fitWidth: Bool
    func pixelLimit(for size: CGSize, combinedAspect: CGFloat? = nil, pair: Bool = false) -> Int {
        let aspect = size.width / size.height
        let totalAspect = combinedAspect ?? aspect
        let width = max(1, viewport.width - (pair ? 3 : 0))
        let height = fitWidth ? width / totalAspect : min(viewport.height, width / totalAspect)
        let longest = max(height, height * aspect) * scale * zoom
        return max(1, min(8192, Int(max(size.width, size.height)), Int(longest.rounded(.up))))
    }
}

private struct PreparedPage {
    let source: CGImageSource
    let size: CGSize
    let name: String
    var landscape: Bool { isLandscape(size) }
}

private struct LoadedPages {
    let images: [PageImage]
    let indices: [Int]
    let error: String?
    let cacheHit: Bool
    let cacheBytes: Int
}

private final class BookWorker {
    private let queue = DispatchQueue(label: "com.suaview.reader", qos: .userInitiated, autoreleaseFrequency: .workItem)
    private let lock = NSLock()
    private var generation = 0
    private var request = 0
    private var book: OpaquePointer?
    private var cache: [Int: PageImage] = [:]
    private var cacheOrder: [Int] = []
    private var cacheBytes = 0
    private let memoryLimit = 192 * 1024 * 1024
    private var count = 0
    private var aspects: [Int: Bool] = [:]
    private var pageSizes: [Int: CGSize] = [:]
    private var invalidAspects: Set<Int> = []

    private func nextGeneration() -> Int {
        lock.lock(); defer { lock.unlock() }
        generation += 1; request += 1
        return generation
    }
    private func nextRequest() -> (Int, Int) {
        lock.lock(); defer { lock.unlock() }
        request += 1; return (generation, request)
    }
    private func valid(_ token: (Int, Int)) -> Bool {
        lock.lock(); defer { lock.unlock() }
        return generation == token.0 && request == token.1
    }
    private func validGeneration(_ value: Int) -> Bool {
        lock.lock(); defer { lock.unlock() }
        return generation == value
    }
    func open(_ url: URL, completion: @escaping (Result<(Int, Int), Error>) -> Void) {
        let token = nextGeneration()
        queue.async { [self] in
            guard validGeneration(token) else { return }
            if let old = book { sua_book_close(old) }
            book = nil; cache.removeAll(); cacheOrder.removeAll(); cacheBytes = 0; count = 0; aspects.removeAll(); pageSizes.removeAll(); invalidAspects.removeAll()
            var error: UnsafeMutablePointer<CChar>?
            let opened = url.path.withCString { sua_book_open($0, &error) }
            guard let opened = opened else {
                let message = error.map { String(cString: $0) } ?? "책을 열 수 없습니다."
                if let error = error { sua_string_free(error) }
                DispatchQueue.main.async { if self.validGeneration(token) { completion(.failure(ReaderError(message))) } }
                return
            }
            book = opened
            count = Int(sua_book_count(opened))
            let initial = Int(sua_book_initial_page(opened))
            let result = (count, initial)
            DispatchQueue.main.async { if self.validGeneration(token) { completion(.success(result)) } }
        }
    }
    func close() {
        _ = nextGeneration()
        queue.async { [self] in
            if let book = book { sua_book_close(book) }
            book = nil; cache.removeAll(); cacheOrder.removeAll(); cacheBytes = 0; count = 0; aspects.removeAll(); pageSizes.removeAll(); invalidAspects.removeAll()
        }
    }
    private func touch(_ index: Int) {
        cacheOrder.removeAll { $0 == index }; cacheOrder.append(index)
    }
    private func insert(_ page: PageImage, protecting indices: Set<Int>) {
        if let old = cache[page.index] { cacheBytes -= old.cost }
        cache[page.index] = page; cacheBytes += page.cost; touch(page.index)
        while cacheBytes > memoryLimit, let victim = cacheOrder.first(where: { !indices.contains($0) }) {
            if let old = cache.removeValue(forKey: victim) { cacheBytes -= old.cost }
            cacheOrder.removeAll { $0 == victim }
        }
    }
    private func retainNearbyPages(around index: Int) {
        let lower = max(0, index - 3), upper = min(count - 1, index + 6)
        for page in cache.keys.filter({ $0 < lower || $0 > upper }) {
            if let removed = cache.removeValue(forKey: page) { cacheBytes -= removed.cost }
        }
        cacheOrder.removeAll { cache[$0] == nil }
    }
    private func prepare(_ index: Int) throws -> PreparedPage {
        guard let book = book else { throw ReaderError("열린 책이 없습니다.") }
        var length = 0
        var error: UnsafeMutablePointer<CChar>?
        guard let bytes = sua_book_read_page(book, index, &length, &error) else {
            let message = error.map { String(cString: $0) } ?? "페이지를 읽을 수 없습니다."
            if let error = error { sua_string_free(error) }
            throw ReaderError(message)
        }
        // ImageIO retains its CFData. Transfer ownership so its backing Rust bytes live exactly as long.
        let data = Data(bytesNoCopy: bytes, count: length, deallocator: .custom { pointer, count in
            sua_bytes_free(pointer.assumingMemoryBound(to: UInt8.self), count)
        })
        guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.doubleValue,
              let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.doubleValue else {
            throw ReaderError("지원하지 않거나 손상된 이미지입니다.")
        }
        guard width > 0, height > 0, width <= 1_000_000, height <= 1_000_000, width * height <= 250_000_000 else { throw ReaderError("이미지 크기가 올바르지 않습니다.") }
        let orientation = (properties[kCGImagePropertyOrientation] as? NSNumber)?.intValue ?? 1
        let original = (5...8).contains(orientation) ? CGSize(width: height, height: width) : CGSize(width: width, height: height)
        let namePointer = sua_book_page_name(book, index)
        let name = namePointer.map { String(cString: $0) } ?? "\(index + 1)"
        if let pointer = namePointer { sua_string_free(pointer) }
        pageSizes[index] = original; aspects[index] = isLandscape(original)
        return PreparedPage(source: source, size: original, name: name)
    }
    private func decode(_ index: Int, pixels: Int, protecting: Set<Int>, prepared: PreparedPage? = nil) throws -> (PageImage, Bool) {
        if let cached = cache[index], cached.pixelLimit >= pixels {
            touch(index); return (cached, true)
        }
        let input = try prepared ?? prepare(index)
        let width = input.size.width, height = input.size.height
        var boundedPixels = min(pixels, Int((max(width, height) * min(1, sqrt(20_000_000 / (width * height)))).rounded(.down)))
        var options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: max(1, boundedPixels),
            kCGImageSourceShouldCacheImmediately: true
        ]
        guard var image = CGImageSourceCreateThumbnailAtIndex(input.source, 0, options as CFDictionary) else {
            throw ReaderError("이미지를 디코딩할 수 없습니다.")
        }
        // ImageIO may preserve 16-bit channels; budget actual row bytes as well as pixel count.
        let pageMemoryLimit = 80 * 1024 * 1024
        if image.bytesPerRow * image.height > pageMemoryLimit {
            boundedPixels = max(1, Int(Double(boundedPixels) * sqrt(Double(pageMemoryLimit) / Double(image.bytesPerRow * image.height)) * 0.94))
            options[kCGImageSourceThumbnailMaxPixelSize] = boundedPixels
            guard let smaller = CGImageSourceCreateThumbnailAtIndex(input.source, 0, options as CFDictionary), smaller.bytesPerRow * smaller.height <= pageMemoryLimit else {
                throw ReaderError("이미지가 메모리 한도를 초과합니다.")
            }
            image = smaller
        }
        let page = PageImage(index: index, name: input.name, image: image, originalSize: input.size, pixelLimit: pixels)
        insert(page, protecting: protecting)
        return (page, false)
    }
    func load(_ index: Int, spread: Bool, cover: Bool, target: DecodeTarget, endBefore: Int? = nil, completion: @escaping (LoadedPages) -> Void) {
        let token = nextRequest()
        queue.async { [self] in
            guard valid(token) else { return }
            retainNearbyPages(around: index)
            var images: [PageImage] = []
            var indices = [index]
            var prepared: [Int: PreparedPage] = [:]
            var failure: String?
            var allCached = true
            func size(_ page: Int) throws -> CGSize {
                if let known = pageSizes[page] { return known }
                let input = try prepare(page); prepared[page] = input
                return input.size
            }
            do {
                let firstSize = try size(index)
                guard valid(token) else { return }
                let firstLandscape = isLandscape(firstSize)
                let end = min(count, endBefore ?? count)
                if canPair(index, end: end, spread: spread, cover: cover, firstLandscape: firstLandscape) {
                    do {
                        let secondSize = try size(index + 1)
                        if canPair(index, end: end, spread: spread, cover: cover, firstLandscape: firstLandscape, secondLandscape: isLandscape(secondSize)) { indices.append(index + 1) }
                    } catch { failure = error.localizedDescription }
                }
                let totalAspect = indices.reduce(CGFloat(0)) { partial, page in
                    let size = pageSizes[page]!
                    return partial + size.width / size.height
                }
                for page in indices {
                    guard valid(token) else { return }
                    let pixels = target.pixelLimit(for: pageSizes[page]!, combinedAspect: totalAspect, pair: indices.count == 2)
                    let decoded = try decode(page, pixels: pixels, protecting: Set(indices), prepared: prepared[page])
                    images.append(decoded.0); allCached = allCached && decoded.1
                }
            } catch { failure = error.localizedDescription }
            guard valid(token) else { return }
            let consumed = images.isEmpty ? [index] : images.map { $0.index }
            let result = LoadedPages(images: images, indices: consumed, error: failure, cacheHit: allCached, cacheBytes: cacheBytes)
            DispatchQueue.main.async { if self.valid(token) { completion(result) } }
            prefetch(around: index, target: target, token: token, protecting: Set(consumed))
        }
    }
    // Read only image metadata when a restored/jumped page has no navigation history.
    private func landscapeAspect(_ index: Int) -> Bool? {
        if let known = aspects[index] { return known }
        guard !invalidAspects.contains(index) else { return nil }
        do { return try prepare(index).landscape }
        catch { invalidAspects.insert(index); return nil }
    }
    func previousStart(before index: Int, cover: Bool, completion: @escaping (Int) -> Void) {
        let token = nextRequest()
        queue.async { [self] in
            guard valid(token), index > 0 else { return }
            let previous = index - 1
            let previousAspect = landscapeAspect(previous)
            let earlierAspect = previous > 0 && previousAspect == false ? landscapeAspect(previous - 1) : nil
            let destination = sua_previous_start(index, 1, cover ? 1 : 0,
                previousAspect.map { $0 ? Int32(1) : Int32(0) } ?? -1,
                earlierAspect.map { $0 ? Int32(1) : Int32(0) } ?? -1)
            DispatchQueue.main.async { if self.valid(token) { completion(destination) } }
        }
    }
    private func prefetch(around index: Int, target: DecodeTarget, token: (Int, Int), protecting: Set<Int>) {
        let candidates = [index + 1, index + 2, index - 1, index + 3, index - 2].filter { $0 >= 0 && $0 < count && !protecting.contains($0) }
        for candidate in candidates {
            guard valid(token) else { return }
            autoreleasepool {
                let prepared = pageSizes[candidate] == nil ? try? prepare(candidate) : nil
                guard let size = pageSizes[candidate] else { return }
                _ = try? decode(candidate, pixels: target.pixelLimit(for: size), protecting: protecting, prepared: prepared)
            }
        }
    }
}

private struct ReaderError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

private final class BookMarkView: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        let color = ReaderAppearance.color("bookMark", NSColor(calibratedRed: 0.72, green: 0.79, blue: 0.86, alpha: 1))
        color.setFill()
        let w = bounds.width, h = bounds.height
        let left = ReaderAppearance.points("markLeft", [CGPoint(x: 0.48, y: 0.2), CGPoint(x: 0.14, y: 0.12), CGPoint(x: 0.14, y: 0.75), CGPoint(x: 0.48, y: 0.84)])
        let right = ReaderAppearance.points("markRight", [CGPoint(x: 0.53, y: 0.2), CGPoint(x: 0.87, y: 0.12), CGPoint(x: 0.87, y: 0.75), CGPoint(x: 0.53, y: 0.84)])
        for points in [left, right] {
            let path = NSBezierPath(); path.move(to: CGPoint(x: points[0].x * w, y: points[0].y * h))
            for point in points.dropFirst() { path.line(to: CGPoint(x: point.x * w, y: point.y * h)) }
            path.close(); path.fill()
        }
    }
}

private final class ReaderCanvas: NSView {
    weak var reader: ReaderController?
    var pages: [PageImage] = [] { didSet { needsDisplay = true } }
    var rightToLeft = false { didSet { needsDisplay = true } }
    var fitWidth = false
    var zoom: CGFloat = 1
    var pan = CGPoint.zero
    var message: String?
    var loading = false
    private var downPoint: NSPoint?
    private var didDrag = false
    private var lastWheelTurn: TimeInterval = 0
    private var wheelTravel: CGFloat = 0
    private var tracking: NSTrackingArea?

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        registerForDraggedTypes([.fileURL])
        setAccessibilityElement(true)
        setAccessibilityRole(.image)
        setAccessibilityLabel("만화책 페이지")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:)") }
    override func updateTrackingAreas() {
        if let tracking = tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: bounds, options: [.activeInKeyWindow, .mouseMoved, .mouseEnteredAndExited, .inVisibleRect], owner: self)
        addTrackingArea(area); tracking = area; super.updateTrackingAreas()
    }
    override func mouseMoved(with event: NSEvent) { reader?.revealHUD() }
    override func mouseEntered(with event: NSEvent) { reader?.revealHUD() }
    override func mouseExited(with event: NSEvent) { reader?.scheduleHideHUD() }
    override func keyDown(with event: NSEvent) {
        if reader?.handleKey(event) != true { super.keyDown(with: event) }
    }
    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        sender.draggingPasteboard.canReadObject(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) ? .copy : []
    }
    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        guard let urls = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL], let url = urls.first else { return false }
        reader?.open(url); return true
    }
    private func imageRects() -> [CGRect] {
        guard !pages.isEmpty else { return [] }
        let ordered = rightToLeft ? Array(pages.reversed()) : pages
        let gutter: CGFloat = ordered.count == 2 ? 3 : 0
        let top: CGFloat = window?.styleMask.contains(.fullScreen) == true ? 8 : 38
        let available = CGRect(x: 12, y: top, width: max(1, bounds.width - 24), height: max(1, bounds.height - top - 12))
        let totalRatio = ordered.reduce(CGFloat(0)) { $0 + $1.originalSize.width / $1.originalSize.height }
        let baseHeight = fitWidth ? (available.width - gutter) / totalRatio : min(available.height, (available.width - gutter) / totalRatio)
        let height = baseHeight * zoom
        let totalWidth = totalRatio * height + gutter
        let x = available.midX - totalWidth / 2 + pan.x
        let y = available.midY - height / 2 + pan.y
        var cursor = x
        return ordered.map { page in
            let width = height * page.originalSize.width / page.originalSize.height
            defer { cursor += width + gutter }
            return CGRect(x: cursor, y: y, width: width, height: height)
        }
    }
    private func clampPan() {
        let rects = imageRects()
        guard let first = rects.first else { pan = .zero; return }
        let total = rects.dropFirst().reduce(first) { $0.union($1) }
        let maxX = max(0, (total.width - bounds.width + 24) / 2)
        let maxY = max(0, (total.height - bounds.height + 48) / 2)
        pan.x = min(maxX, max(-maxX, pan.x)); pan.y = min(maxY, max(-maxY, pan.y))
    }
    func clampPosition() { clampPan(); needsDisplay = true }
    func resetPosition() {
        pan = .zero
        if fitWidth, let rect = imageRects().first { pan.y = max(0, rect.height - bounds.height + 48) / 2 }
        needsDisplay = true
    }
    override func draw(_ dirtyRect: NSRect) {
        ReaderAppearance.color("canvas", NSColor(calibratedRed: 0.071, green: 0.075, blue: 0.086, alpha: 1)).setFill(); bounds.fill()
        guard !pages.isEmpty else {
            if let message = message {
                let style = NSMutableParagraphStyle(); style.alignment = .center
                let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 14), .foregroundColor: NSColor.secondaryLabelColor, .paragraphStyle: style]
                let width = min(bounds.width - 70, 680)
                (message as NSString).draw(in: NSRect(x: (bounds.width - width) / 2, y: bounds.midY - 35, width: width, height: 150), withAttributes: attributes)
            }
            return
        }
        let ordered = rightToLeft ? Array(pages.reversed()) : pages
        let rects = imageRects()
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        context.interpolationQuality = .high
        for (page, rect) in zip(ordered, rects) {
            context.saveGState()
            context.translateBy(x: rect.minX, y: rect.maxY)
            context.scaleBy(x: 1, y: -1)
            context.draw(page.image, in: CGRect(origin: .zero, size: rect.size))
            context.restoreGState()
        }
    }
    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        downPoint = convert(event.locationInWindow, from: nil); didDrag = false
        reader?.revealHUD()
    }
    override func mouseDragged(with event: NSEvent) {
        guard let down = downPoint else { return }
        let point = convert(event.locationInWindow, from: nil)
        if abs(point.x - down.x) + abs(point.y - down.y) > 4 { didDrag = true }
        if didDrag { pan.x += event.deltaX; pan.y += event.deltaY; clampPan(); needsDisplay = true }
    }
    override func mouseUp(with event: NSEvent) {
        defer { downPoint = nil }
        guard !didDrag, !pages.isEmpty else { return }
        if event.clickCount == 2 { reader?.toggleFullscreen(nil); return }
        let x = convert(event.locationInWindow, from: nil).x
        if x < bounds.width * 0.35 { reader?.turn(rightToLeft ? 1 : -1) }
        else if x > bounds.width * 0.65 { reader?.turn(rightToLeft ? -1 : 1) }
        else { reader?.revealHUD() }
    }
    override func scrollWheel(with event: NSEvent) {
        guard !pages.isEmpty else { return }
        reader?.revealHUD()
        let scrollable = imageRects().contains { $0.height > bounds.height + 8 || $0.width > bounds.width + 8 }
        if scrollable {
            pan.x += event.scrollingDeltaX; pan.y += event.scrollingDeltaY; clampPan(); needsDisplay = true
            return
        }
        guard event.momentumPhase == [] else { return }
        if event.phase == .began { wheelTravel = 0 }
        let delta = abs(event.scrollingDeltaX) > abs(event.scrollingDeltaY) ? event.scrollingDeltaX : event.scrollingDeltaY
        wheelTravel += delta
        let threshold: CGFloat = event.hasPreciseScrollingDeltas ? 65 : 2
        if abs(wheelTravel) >= threshold && event.timestamp - lastWheelTurn > 0.32 {
            reader?.turn(wheelTravel < 0 ? 1 : -1); wheelTravel = 0; lastWheelTurn = event.timestamp
        }
    }
    override func magnify(with event: NSEvent) {
        zoom = min(8, max(0.3, zoom * (1 + event.magnification))); clampPan(); needsDisplay = true
        if event.phase == .ended { reader?.refreshForZoom() }
    }
}

private final class ReaderController: NSWindowController, NSWindowDelegate, NSMenuItemValidation {
    private let worker = BookWorker()
    private let defaults = UserDefaults.standard
    private let canvas = ReaderCanvas(frame: .zero)
    private let empty = NSStackView()
    private let hud = NSVisualEffectView()
    private let pageButton = NSButton(title: "", target: nil, action: nil)
    private let slider = NSSlider(value: 0, minValue: 0, maxValue: 1, target: nil, action: nil)
    private let nameLabel = NSTextField(labelWithString: "")
    private let busy = NSProgressIndicator()
    private var hideHUD: DispatchWorkItem?
    private var resizeWork: DispatchWorkItem?
    private var url: URL?
    private var count = 0
    private var index = 0
    private var displayedCount = 1
    private var spanEndExclusive: Int?
    private var pendingTurns: [Int] = []
    private var navigationRevision = 0
    private var displayStarts: [Int] = []
    private var isOpening = false
    private var isLoading = false
    private var isResolvingPrevious = false
    private var firstPage = true
    private var smokeStarted = false
    private var smoke = false
    private var stress = false
    private var stressPhase = "single"
    private var stressExpected = 0
    private var stressSingleVisited: Set<Int> = []
    private var stressSpreadVisited: Set<Int> = []
    private var stressLandscapes: Set<Int> = []
    private var stressSpreadStarts: [Int] = []
    private var stressMixedExpected: [Int] = []
    private var stressProbeTargets: [Int] = []
    private var stressProbeTarget = 0
    private var stressPreviousExpected = 0
    private var stressAssertions = 0
    private var stressMaxCache = 0
    private var requestStarted = DispatchTime.now().uptimeNanoseconds
    private var rightToLeft: Bool
    private var spread: Bool
    private var coverAlone: Bool
    private var fitWidth: Bool

    init() {
        rightToLeft = testing ? false : UserDefaults.standard.bool(forKey: "rightToLeft")
        spread = testing ? false : UserDefaults.standard.bool(forKey: "spread")
        coverAlone = testing || UserDefaults.standard.object(forKey: "coverAlone") == nil ? true : UserDefaults.standard.bool(forKey: "coverAlone")
        fitWidth = testing ? false : UserDefaults.standard.bool(forKey: "fitWidth")
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1080, height: 790), styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.title = "Sua View"
        window.titlebarAppearsTransparent = true
        window.backgroundColor = ReaderAppearance.color("canvas", NSColor(calibratedRed: 0.071, green: 0.075, blue: 0.086, alpha: 1))
        window.appearance = NSAppearance(named: .darkAqua)
        window.minSize = NSSize(width: 520, height: 420)
        window.isReleasedWhenClosed = false
        window.tabbingMode = .disallowed
        super.init(window: window)
        window.delegate = self
        window.center()
        configureContent()
        canvas.reader = self; canvas.rightToLeft = rightToLeft; canvas.fitWidth = fitWidth
    }
    required init?(coder: NSCoder) { fatalError("init(coder:)") }

    private func configureContent() {
        guard let content = window?.contentView else { return }
        canvas.translatesAutoresizingMaskIntoConstraints = false; content.addSubview(canvas)
        NSLayoutConstraint.activate([canvas.leadingAnchor.constraint(equalTo: content.leadingAnchor), canvas.trailingAnchor.constraint(equalTo: content.trailingAnchor), canvas.topAnchor.constraint(equalTo: content.topAnchor), canvas.bottomAnchor.constraint(equalTo: content.bottomAnchor)])
        empty.orientation = .vertical; empty.alignment = .centerX; empty.spacing = 15
        empty.translatesAutoresizingMaskIntoConstraints = false; content.addSubview(empty)
        let mark = BookMarkView(frame: .zero); mark.translatesAutoresizingMaskIntoConstraints = false
        mark.widthAnchor.constraint(equalToConstant: ReaderAppearance.metric("markSize", 76)).isActive = true; mark.heightAnchor.constraint(equalToConstant: ReaderAppearance.metric("markSize", 76)).isActive = true
        let title = NSTextField(labelWithString: "Sua View"); title.font = .systemFont(ofSize: ReaderAppearance.metric("titleSize", 29), weight: .semibold); title.textColor = ReaderAppearance.color("textPrimary", .labelColor)
        let subtitle = NSTextField(labelWithString: "만화책을 여기에 놓으세요"); subtitle.font = .systemFont(ofSize: ReaderAppearance.metric("subtitleSize", 14)); subtitle.textColor = ReaderAppearance.color("textSecondary", .secondaryLabelColor)
        let types = NSTextField(labelWithString: "ZIP · CBZ · CBR · 7Z · 이미지 폴더"); types.font = .systemFont(ofSize: ReaderAppearance.metric("detailSize", 11)); types.textColor = ReaderAppearance.color("textMuted", .tertiaryLabelColor)
        let open = NSButton(title: "책 열기…", target: self, action: #selector(openPanel(_:))); open.bezelStyle = .rounded; open.controlSize = .large
        let shortcut = NSTextField(labelWithString: "⌘ O"); shortcut.font = .monospacedSystemFont(ofSize: ReaderAppearance.metric("detailSize", 11), weight: .regular); shortcut.textColor = ReaderAppearance.color("textMuted", .tertiaryLabelColor)
        [mark, title, subtitle, types, open, shortcut].forEach { empty.addArrangedSubview($0) }
        empty.setCustomSpacing(4, after: subtitle); empty.setCustomSpacing(12, after: types); empty.setCustomSpacing(3, after: open)
        NSLayoutConstraint.activate([empty.centerXAnchor.constraint(equalTo: content.centerXAnchor), empty.centerYAnchor.constraint(equalTo: content.centerYAnchor, constant: ReaderAppearance.metric("emptyOffset", -8))])

        hud.material = .hudWindow; hud.blendingMode = .withinWindow; hud.state = .active
        hud.wantsLayer = true; hud.layer?.cornerRadius = ReaderAppearance.metric("hudRadius", 11); hud.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(hud); hud.isHidden = true
        let row = NSStackView(); row.orientation = .horizontal; row.spacing = 11; row.alignment = .centerY; row.translatesAutoresizingMaskIntoConstraints = false; hud.addSubview(row)
        let openButton = symbolButton("folder", label: "다른 책 열기", action: #selector(openPanel(_:)))
        let previous = symbolButton("chevron.left", label: "이전 페이지", action: #selector(previousPage(_:)))
        let next = symbolButton("chevron.right", label: "다음 페이지", action: #selector(nextPage(_:)))
        let options = symbolButton("ellipsis.circle", label: "읽기 옵션", action: #selector(showOptions(_:)))
        pageButton.target = self; pageButton.action = #selector(goToPage(_:)); pageButton.isBordered = false; pageButton.font = .monospacedDigitSystemFont(ofSize: 12, weight: .medium)
        pageButton.setContentHuggingPriority(.required, for: .horizontal)
        pageButton.setAccessibilityLabel("현재 페이지 / 전체 페이지 · 클릭해서 이동")
        pageButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 78).isActive = true
        slider.target = self; slider.action = #selector(slidePage(_:)); slider.isContinuous = false; slider.controlSize = .small
        slider.widthAnchor.constraint(greaterThanOrEqualToConstant: 90).isActive = true
        slider.setAccessibilityLabel("페이지 이동")
        nameLabel.font = .systemFont(ofSize: 11); nameLabel.textColor = .secondaryLabelColor; nameLabel.lineBreakMode = .byTruncatingMiddle
        nameLabel.widthAnchor.constraint(lessThanOrEqualToConstant: 180).isActive = true
        nameLabel.widthAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
        [openButton, previous, slider, pageButton, next, nameLabel, options].forEach { row.addArrangedSubview($0) }
        NSLayoutConstraint.activate([hud.centerXAnchor.constraint(equalTo: content.centerXAnchor), hud.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -ReaderAppearance.metric("hudInset", 12)), hud.widthAnchor.constraint(lessThanOrEqualTo: content.widthAnchor, constant: -28), hud.widthAnchor.constraint(equalToConstant: ReaderAppearance.metric("hudWidth", 680)).withPriority(.defaultHigh), row.leadingAnchor.constraint(equalTo: hud.leadingAnchor, constant: 14), row.trailingAnchor.constraint(equalTo: hud.trailingAnchor, constant: -14), row.topAnchor.constraint(equalTo: hud.topAnchor, constant: 9), row.bottomAnchor.constraint(equalTo: hud.bottomAnchor, constant: -9)])
        busy.style = .spinning; busy.controlSize = .regular; busy.isDisplayedWhenStopped = false; busy.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(busy); NSLayoutConstraint.activate([busy.centerXAnchor.constraint(equalTo: content.centerXAnchor), busy.centerYAnchor.constraint(equalTo: content.centerYAnchor)])
    }
    private func symbolButton(_ symbol: String, label: String, action: Selector) -> NSButton {
        let button = NSButton(image: NSImage(systemSymbolName: symbol, accessibilityDescription: label) ?? NSImage(), target: self, action: action)
        button.isBordered = false; button.toolTip = label; button.setAccessibilityLabel(label)
        button.widthAnchor.constraint(equalToConstant: 25).isActive = true
        return button
    }
    func activate() {
        showWindow(nil); window?.makeKeyAndOrderFront(nil); window?.makeFirstResponder(canvas)
        NSApp.activate(ignoringOtherApps: true)
    }
    func open(_ newURL: URL) {
        savePosition()
        url = newURL.standardizedFileURL
        resetNavigation(); isLoading = false
        isOpening = true; firstPage = true; count = 0; index = 0; displayedCount = 1; displayStarts = []
        canvas.pages = []; canvas.message = nil; canvas.zoom = 1; canvas.pan = .zero
        empty.isHidden = true; hud.isHidden = true; busy.startAnimation(nil)
        window?.title = "\(newURL.lastPathComponent) — Sua View"
        activate()
        worker.open(newURL) { [weak self] result in
            guard let self = self else { return }
            self.isOpening = false
            switch result {
            case .failure(let error):
                self.busy.stopAnimation(nil); self.canvas.message = "책을 열 수 없습니다.\n\n\(error.localizedDescription)\n\n⌘ O로 다른 책을 여세요."; self.canvas.needsDisplay = true
                self.url = nil; self.window?.title = "Sua View"
                metric("open_error", ["message": error.localizedDescription])
                if self.stress { _ = self.stressCheck(false, "Book open failed") }
            case .success(let value):
                self.count = value.0
                let saved = self.savedPosition(for: newURL)
                let isImage = newURL.path.withCString { sua_path_is_image($0) != 0 }
                self.index = self.stress ? 0 : (isImage ? value.1 : min(self.count - 1, max(0, saved)))
                self.slider.maxValue = Double(max(1, self.count - 1)); self.slider.minValue = 0
                metric("opened", ["count": self.count, "index": self.index])
                if !testing { NSDocumentController.shared.noteNewRecentDocumentURL(newURL) }
                self.requestPages(resetPosition: true)
            }
        }
    }
    private func decodeTarget() -> DecodeTarget {
        let top: CGFloat = window?.styleMask.contains(.fullScreen) == true ? 8 : 38
        return DecodeTarget(viewport: CGSize(width: max(1, canvas.bounds.width - 24), height: max(1, canvas.bounds.height - top - 12)), scale: window?.backingScaleFactor ?? 2, zoom: canvas.zoom, fitWidth: fitWidth)
    }
    private func requestPages(resetPosition: Bool) {
        guard count > 0, !isOpening, !isResolvingPrevious else { return }
        isLoading = true; requestStarted = DispatchTime.now().uptimeNanoseconds
        let requestedIndex = index
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) { [weak self] in
            guard let self = self, self.isLoading, self.index == requestedIndex else { return }; self.busy.startAnimation(nil)
        }
        worker.load(index, spread: spread, cover: coverAlone, target: decodeTarget(), endBefore: spanEndExclusive) { [weak self] result in
            guard let self = self else { return }
            self.isLoading = false; self.busy.stopAnimation(nil)
            self.canvas.pages = result.images; self.canvas.message = result.images.isEmpty ? "페이지를 표시할 수 없습니다.\n\n\(result.error ?? "이미지를 읽을 수 없습니다.")\n\n방향키로 다음 페이지로 이동할 수 있습니다." : nil
            self.displayedCount = result.indices.count
            if resetPosition { self.canvas.resetPosition() }
            self.updateHUD(); self.revealHUD(); self.savePosition()
            self.canvas.layoutSubtreeIfNeeded(); self.canvas.displayIfNeeded()
            self.canvas.setAccessibilityLabel("\(self.index + 1) / \(self.count) 페이지")
            self.window?.subtitle = "\(self.index + 1) / \(self.count)"
            metric(self.firstPage ? "first_page" : "page", ["index": self.index, "count": self.count, "displayed": result.images.count, "cache_hit": result.cacheHit, "request_ms": Double(DispatchTime.now().uptimeNanoseconds - self.requestStarted) / 1_000_000, "image_width": result.images.first?.image.width ?? 0, "image_height": result.images.first?.image.height ?? 0, "cache_bytes": result.cacheBytes])
            if let error = result.error { metric("page_error", ["index": self.index, "message": error]) }
            self.firstPage = false
            if self.stress { self.stressDidLoad(result) }
            else if self.smoke && !self.smokeStarted { self.startSmoke() }
            self.drainTurns()
        }
    }
    private func updateHUD() {
        let last = min(count, index + displayedCount)
        pageButton.title = displayedCount > 1 ? "\(index + 1)–\(last) / \(count)" : "\(index + 1) / \(count)"
        slider.doubleValue = Double(index); slider.isEnabled = count > 1
        nameLabel.stringValue = canvas.pages.first?.name ?? ""
        nameLabel.toolTip = nameLabel.stringValue
    }
    func revealHUD() {
        guard count > 0 else { return }
        hideHUD?.cancel(); hud.isHidden = false
        hud.alphaValue = 1; scheduleHideHUD()
    }
    func scheduleHideHUD() {
        hideHUD?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self = self, let window = self.window else { return }
            let point = self.hud.convert(window.mouseLocationOutsideOfEventStream, from: nil)
            guard !self.hud.bounds.contains(point) else { self.scheduleHideHUD(); return }
            NSAnimationContext.runAnimationGroup { context in context.duration = 0.18; self.hud.animator().alphaValue = 0 }
        }
        hideHUD = item; DispatchQueue.main.asyncAfter(deadline: .now() + Double(ReaderAppearance.metric("hudHideSeconds", 2)), execute: item)
    }
    private func resetNavigation() {
        pendingTurns.removeAll(); spanEndExclusive = nil; isResolvingPrevious = false; navigationRevision += 1
    }
    private func drainTurns() {
        while !isLoading, !pendingTurns.isEmpty { turn(pendingTurns.removeFirst()) }
    }
    func turn(_ direction: Int) {
        guard count > 0, !isOpening else { return }
        if isLoading {
            if pendingTurns.count < 32 { pendingTurns.append(direction > 0 ? 1 : -1) }
            return
        }
        if direction > 0 {
            let next = sua_next_start(index, count, displayedCount)
            guard next != index else { NSSound.beep(); return }
            displayStarts.append(index)
            index = next; spanEndExclusive = nil
            requestPages(resetPosition: true)
            return
        }
        guard index > 0 else { NSSound.beep(); return }
        let oldIndex = index
        if let previous = displayStarts.last, previous < index {
            displayStarts.removeLast(); index = previous; spanEndExclusive = oldIndex
            requestPages(resetPosition: true)
        } else if !spread {
            index -= 1; spanEndExclusive = oldIndex; requestPages(resetPosition: true)
        } else {
            isLoading = true; isResolvingPrevious = true
            let revision = navigationRevision
            worker.previousStart(before: index, cover: coverAlone) { [weak self] previous in
                guard let self = self, self.navigationRevision == revision else { return }
                self.isResolvingPrevious = false
                self.index = previous; self.spanEndExclusive = oldIndex
                self.requestPages(resetPosition: true)
            }
        }
    }
    @objc func nextPage(_ sender: Any?) { turn(1) }
    @objc func previousPage(_ sender: Any?) { turn(-1) }
    @objc func firstPage(_ sender: Any?) { jump(0) }
    @objc func lastPage(_ sender: Any?) { jump(count - 1) }
    private func jump(_ value: Int) {
        guard count > 0 else { return }
        let destination = min(count - 1, max(0, value))
        resetNavigation()
        index = destination; displayStarts = []; requestPages(resetPosition: true)
    }
    @objc func slidePage(_ sender: NSSlider) { jump(Int(sender.doubleValue.rounded())) }
    @objc func openPanel(_ sender: Any?) {
        let panel = NSOpenPanel(); panel.canChooseDirectories = true; panel.canChooseFiles = true; panel.allowsMultipleSelection = false
        panel.title = "만화책 열기"; panel.prompt = "열기"
        panel.message = "압축 파일, 이미지 또는 이미지 폴더를 선택하세요."
        panel.beginSheetModal(for: window!) { [weak self] response in if response == .OK, let url = panel.url { self?.open(url) } }
    }
    @objc func closeBook(_ sender: Any?) {
        resetNavigation(); savePosition(); worker.close(); url = nil; count = 0; index = 0; isOpening = false; isLoading = false
        canvas.pages = []; canvas.message = nil; empty.isHidden = false; hud.isHidden = true; busy.stopAnimation(nil)
        window?.title = "Sua View"; window?.subtitle = ""
    }
    @objc func toggleRTL(_ sender: Any?) {
        rightToLeft.toggle(); canvas.rightToLeft = rightToLeft
        persist("rightToLeft", rightToLeft); revealHUD()
        metric("direction", ["rtl": rightToLeft])
    }
    @objc func toggleSpread(_ sender: Any?) {
        resetNavigation(); spread.toggle(); displayStarts = []; persist("spread", spread); requestPages(resetPosition: true)
        metric("spread", ["enabled": spread])
    }
    @objc func toggleCover(_ sender: Any?) { resetNavigation(); coverAlone.toggle(); persist("coverAlone", coverAlone); displayStarts = []; requestPages(resetPosition: true) }
    @objc func fitPage(_ sender: Any?) { fitWidth = false; canvas.fitWidth = false; canvas.zoom = 1; canvas.resetPosition(); persist("fitWidth", false); requestPages(resetPosition: true) }
    @objc func fitToWidth(_ sender: Any?) { fitWidth = true; canvas.fitWidth = true; canvas.zoom = 1; canvas.resetPosition(); persist("fitWidth", true); requestPages(resetPosition: true) }
    @objc func zoomIn(_ sender: Any?) { zoom(by: 1.25) }
    @objc func zoomOut(_ sender: Any?) { zoom(by: 0.8) }
    private func zoom(by factor: CGFloat) { canvas.zoom = min(8, max(0.3, canvas.zoom * factor)); canvas.clampPosition(); refreshForZoom() }
    func refreshForZoom() { requestPages(resetPosition: false) }
    @objc func toggleFullscreen(_ sender: Any?) { window?.toggleFullScreen(sender) }
    @objc func goToPage(_ sender: Any?) {
        guard count > 0 else { return }
        let alert = NSAlert(); alert.messageText = "페이지 이동"; alert.informativeText = "1부터 \(count) 사이의 페이지를 입력하세요."
        let field = NSTextField(string: "\(index + 1)"); field.frame = NSRect(x: 0, y: 0, width: 230, height: 24)
        alert.accessoryView = field; alert.addButton(withTitle: "이동"); alert.addButton(withTitle: "취소")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window!) { [weak self] response in
            if response == .alertFirstButtonReturn, let value = Int(field.stringValue), let self = self {
                if value >= 1 && value <= self.count { self.jump(value - 1) } else { NSSound.beep() }
            }
            self?.window?.makeFirstResponder(self?.canvas)
        }
    }
    @objc func shortcutHelp(_ sender: Any?) {
        let alert = NSAlert(); alert.messageText = "읽기에 필요한 단축키"
        alert.informativeText = "⌘ O    책 열기\n← →    이전 / 다음 (읽기 방향에 따라)\nSpace / Shift Space    다음 / 이전\nHome / End    첫 / 마지막 페이지\nD    두 페이지 보기\nR    읽기 방향 바꾸기\nW / 0    너비 / 화면에 맞추기\n+ / −    확대 / 축소\nG    페이지 이동\nF    전체 화면\nEsc    전체 화면 종료\n\nFinder: 파일 선택 후 ⇧⌘J로 Sua View 열기\n(시스템 설정 → 키보드 → 키보드 단축키 → 서비스에서 변경)"
        alert.addButton(withTitle: "확인"); alert.beginSheetModal(for: window!) { [weak self] _ in self?.window?.makeFirstResponder(self?.canvas) }
    }
    @objc func showOptions(_ sender: NSButton) {
        let menu = NSMenu()
        addOption(menu, "두 페이지 보기", #selector(toggleSpread(_:)), checked: spread)
        addOption(menu, "첫 페이지는 표지 한 장", #selector(toggleCover(_:)), checked: coverAlone)
        addOption(menu, "오른쪽에서 왼쪽으로", #selector(toggleRTL(_:)), checked: rightToLeft)
        menu.addItem(.separator())
        addOption(menu, "화면에 맞추기", #selector(fitPage(_:)), checked: !fitWidth && canvas.zoom == 1)
        addOption(menu, "너비에 맞추기", #selector(fitToWidth(_:)), checked: fitWidth && canvas.zoom == 1)
        addOption(menu, "전체 화면", #selector(toggleFullscreen(_:)), checked: window?.styleMask.contains(.fullScreen) == true)
        menu.addItem(.separator()); addOption(menu, "단축키", #selector(shortcutHelp(_:)))
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.height + 8), in: sender)
    }
    private func addOption(_ menu: NSMenu, _ title: String, _ action: Selector, checked: Bool = false) {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: ""); item.target = self; item.state = checked ? .on : .off; menu.addItem(item)
    }
    func handleKey(_ event: NSEvent) -> Bool {
        if !event.modifierFlags.intersection([.command, .control, .option]).isEmpty { return false }
        switch event.keyCode {
        case 123: turn(rightToLeft ? 1 : -1)
        case 124: turn(rightToLeft ? -1 : 1)
        case 125, 121: turn(1)
        case 126, 116: turn(-1)
        case 115: firstPage(nil)
        case 119: lastPage(nil)
        case 49: turn(event.modifierFlags.contains(.shift) ? -1 : 1)
        // Physical keys remain usable while a Korean input source is active.
        case 2: toggleSpread(nil)
        case 15: toggleRTL(nil)
        case 3: toggleFullscreen(nil)
        case 13: fitToWidth(nil)
        case 29, 82: fitPage(nil)
        case 24, 69: zoomIn(nil)
        case 27, 78: zoomOut(nil)
        case 5: goToPage(nil)
        case 44: shortcutHelp(nil)
        case 53:
            if window?.styleMask.contains(.fullScreen) == true { toggleFullscreen(nil) }
            else { canvas.zoom = 1; canvas.resetPosition(); revealHUD() }
        default:
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "r": toggleRTL(nil)
            case "d": toggleSpread(nil)
            case "f": toggleFullscreen(nil)
            case "w": fitToWidth(nil)
            case "0": fitPage(nil)
            case "+", "=": zoomIn(nil)
            case "-", "_": zoomOut(nil)
            case "g": goToPage(nil)
            case "?", "/": shortcutHelp(nil)
            default: return false
            }
        }
        return true
    }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(toggleRTL(_:)): item.state = rightToLeft ? .on : .off
        case #selector(toggleSpread(_:)): item.state = spread ? .on : .off
        case #selector(toggleCover(_:)): item.state = coverAlone ? .on : .off
        case #selector(fitPage(_:)): item.state = !fitWidth && canvas.zoom == 1 ? .on : .off
        case #selector(fitToWidth(_:)): item.state = fitWidth && canvas.zoom == 1 ? .on : .off
        default: break
        }
        if [#selector(nextPage(_:)), #selector(previousPage(_:)), #selector(firstPage(_:)), #selector(lastPage(_:)), #selector(goToPage(_:)), #selector(closeBook(_:))].contains(item.action) { return count > 0 }
        return true
    }
    private func persist(_ key: String, _ value: Any) { if !testing { defaults.set(value, forKey: key) } }
    private func savedPosition(for url: URL) -> Int {
        guard !testing else { return 0 }
        return (defaults.dictionary(forKey: "positions")?[url.standardizedFileURL.path] as? Int) ?? 0
    }
    func savePosition() {
        guard !testing, let url = url, count > 0 else { return }
        var positions = defaults.dictionary(forKey: "positions") ?? [:]
        var recent = defaults.stringArray(forKey: "positionOrder") ?? []
        let path = url.standardizedFileURL.path
        positions[path] = index; recent.removeAll { $0 == path }; recent.append(path)
        while recent.count > 50 { positions.removeValue(forKey: recent.removeFirst()) }
        defaults.set(positions, forKey: "positions"); defaults.set(recent, forKey: "positionOrder")
    }
    func windowWillClose(_ notification: Notification) { savePosition() }
    func windowDidResize(_ notification: Notification) {
        canvas.needsDisplay = true; resizeWork?.cancel()
        let item = DispatchWorkItem { [weak self] in self?.requestPages(resetPosition: false) }
        resizeWork = item; DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: item)
    }
    func windowDidChangeBackingProperties(_ notification: Notification) { requestPages(resetPosition: false) }
    func setStress() {
        stress = true; spread = false; rightToLeft = false; coverAlone = true; fitWidth = false
        canvas.rightToLeft = false; canvas.fitWidth = false
    }
    private func stressCheck(_ condition: Bool, _ message: String) -> Bool {
        stressAssertions += 1
        guard condition else {
            metric("stress_failed", ["reason": message, "phase": stressPhase, "index": index, "assertions": stressAssertions])
            fputs("Sua View stress failure: \(message)\n", stderr)
            exit(2)
        }
        return true
    }
    private func stressDidLoad(_ result: LoadedPages) {
        stressMaxCache = max(stressMaxCache, result.cacheBytes)
        guard stressCheck(result.error == nil && !result.images.isEmpty, "Page decode failed"),
              stressCheck(result.cacheBytes <= 192 * 1024 * 1024, "Decoded cache exceeded its bound") else { return }
        if stressPhase == "single" {
            guard stressCheck(index == stressExpected, "Single-page traversal skipped an index"),
                  stressCheck(result.indices == [index], "Single-page layout consumed extra pages") else { return }
            stressSingleVisited.insert(index)
            for page in result.images where page.landscape { stressLandscapes.insert(page.index) }
            if index + 1 < count { stressExpected = index + 1; turn(1) }
            else {
                stressPhase = "spread"; stressExpected = 0; spread = true
                resetNavigation(); displayStarts = []; index = 0; requestPages(resetPosition: true)
            }
        } else if stressPhase == "spread" {
            guard stressCheck(index == stressExpected, "Rapid spread navigation skipped or duplicated pages") else { return }
            stressSpreadStarts.append(index)
            for page in result.indices {
                guard stressCheck(!stressSpreadVisited.contains(page), "Spread traversal displayed a page twice") else { return }
                stressSpreadVisited.insert(page)
            }
            stressExpected = index + displayedCount
            if stressExpected >= count {
                guard stressCheck(stressSpreadVisited.count == count && stressSingleVisited.count == count, "Traversal did not cover every page") else { return }
                resetNavigation()
                var probes: Set<Int> = [1, 2, 3, count - 1]
                for landscape in stressLandscapes { for offset in 0...3 { probes.insert(landscape + offset) } }
                stressProbeTargets = probes.filter { $0 > 0 && $0 < count }.sorted()
                if stressSpreadStarts.count >= 3 {
                    stressPhase = "mixed_before"; displayStarts = []; index = 0
                    requestPages(resetPosition: true)
                } else { nextStressProbe() }
            } else if pendingTurns.isEmpty {
                // Four key presses land in the queue before the next layout is known.
                for _ in 0..<4 { turn(1) }
            }
        } else if stressPhase == "mixed_before" {
            stressPhase = "mixed"
            stressMixedExpected = [stressSpreadStarts[1], stressSpreadStarts[2], stressSpreadStarts[1]]
            for intent in [1, 1, -1] { turn(intent) }
        } else if stressPhase == "mixed" {
            guard let expected = stressMixedExpected.first,
                  stressCheck(index == expected, "Rapid next/next/previous disagreed with sequential navigation") else { return }
            stressMixedExpected.removeFirst()
            if stressMixedExpected.isEmpty { nextStressProbe() }
        } else if stressPhase == "probe_before" {
            guard stressCheck(index == stressProbeTarget, "Jump landed at the wrong index") else { return }
            let previous = index - 1
            let earlier = previous - 1
            stressPreviousExpected = previous
            if previous > 0, !stressLandscapes.contains(previous), !(coverAlone && earlier == 0), !stressLandscapes.contains(earlier) { stressPreviousExpected = earlier }
            stressPhase = "probe_after"; turn(-1)
        } else if stressPhase == "probe_after" {
            guard stressCheck(index == stressPreviousExpected, "Previous skipped a landscape or cover singleton"),
                  stressCheck(result.indices.allSatisfy { $0 < stressProbeTarget }, "Previous overlapped the current spread") else { return }
            stressPhase = "probe_return"; turn(1)
        } else if stressPhase == "probe_return" {
            guard stressCheck(index == stressProbeTarget, "Next failed to reverse a previous jump") else { return }
            nextStressProbe()
        }
    }
    private func nextStressProbe() {
        guard !stressProbeTargets.isEmpty else {
            stress = false
            metric("stress_complete", ["count": count, "single_visited": stressSingleVisited.count, "spread_visited": stressSpreadVisited.count, "assertions": stressAssertions, "max_cache_bytes": stressMaxCache])
            NSApp.terminate(nil); return
        }
        stressProbeTarget = stressProbeTargets.removeFirst()
        stressPhase = "probe_before"; resetNavigation(); displayStarts = []
        index = stressProbeTarget; requestPages(resetPosition: true)
    }
    func setSmoke() { smoke = true }
    private func startSmoke() {
        smokeStarted = true
        let actions: [() -> Void] = [
            { self.turn(1) }, { self.turn(-1) }, { self.toggleSpread(nil) },
            { self.turn(1) }, { self.toggleRTL(nil) }, { self.fitToWidth(nil) }, { self.fitPage(nil) },
            { metric("smoke_complete", ["count": self.count, "index": self.index]); if CommandLine.arguments.contains("--smoke-exit") { NSApp.terminate(nil) } }
        ]
        for (offset, action) in actions.enumerated() { DispatchQueue.main.asyncAfter(deadline: .now() + Double(offset + 1) * 0.45, execute: action) }
    }
}

private extension NSLayoutConstraint {
    func withPriority(_ value: NSLayoutConstraint.Priority) -> NSLayoutConstraint { priority = value; return self }
}

private final class AppDelegate: NSObject, NSApplicationDelegate {
    var reader: ReaderController!
    private var pendingURLs: [URL] = []
    func applicationWillFinishLaunching(_ notification: Notification) {
        NSApp.servicesProvider = self
        reader = ReaderController(); createMenus()
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSUpdateDynamicServices()
        reader.activate()
        let args = CommandLine.arguments
        if let stressIndex = args.firstIndex(of: "--stress-test"), args.count > stressIndex + 1 {
            reader.setStress(); reader.open(URL(fileURLWithPath: args[stressIndex + 1]))
        } else if let smokeIndex = args.firstIndex(of: "--smoke-test"), args.count > smokeIndex + 1 {
            reader.setSmoke(); reader.open(URL(fileURLWithPath: args[smokeIndex + 1]))
        } else if let url = pendingURLs.last { reader.open(url); pendingURLs = [] }
        else if let path = args.dropFirst().first(where: { !$0.hasPrefix("-") }) { reader.open(URL(fileURLWithPath: path)) }
        metric("ready")
    }
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        if let path = filenames.first {
            if reader != nil { reader.open(URL(fileURLWithPath: path)) } else { pendingURLs.append(URL(fileURLWithPath: path)) }
        }
        sender.reply(toOpenOrPrint: .success)
    }
    func application(_ application: NSApplication, open urls: [URL]) {
        if let url = urls.first { if reader != nil { reader.open(url) } else { pendingURLs.append(url) } }
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool { reader.activate(); return true }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { reader?.savePosition() }
    @objc func openBook(_ pboard: NSPasteboard, userData: String?, error: AutoreleasingUnsafeMutablePointer<NSString?>) {
        let urls = pboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]
        let legacy = pboard.propertyList(forType: NSPasteboard.PasteboardType("NSFilenamesPboardType")) as? [String]
        guard let url = urls?.first ?? legacy?.first.map({ URL(fileURLWithPath: $0) }) else { error.pointee = "선택한 파일을 읽을 수 없습니다."; return }
        reader.activate(); reader.open(url)
    }
    @objc func reopenRecent(_ sender: NSMenuItem) { if let url = sender.representedObject as? URL { reader.open(url) } }
    @objc func clearRecent(_ sender: Any?) { NSDocumentController.shared.clearRecentDocuments(sender) }
    @objc func showAbout(_ sender: Any?) {
        NSApp.orderFrontStandardAboutPanel(options: [.applicationName: "Sua View", .applicationVersion: "1.0", .credits: NSAttributedString(string: "빠르고 가벼운 만화책 뷰어\nRust · macOS AppKit")])
    }
    private func menuItem(_ title: String, _ action: Selector, _ key: String = "", target: AnyObject? = nil, modifiers: NSEvent.ModifierFlags = .command) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key); item.target = target ?? reader; item.keyEquivalentModifierMask = modifiers; return item
    }
    private func createMenus() {
        let main = NSMenu(); NSApp.mainMenu = main
        let appItem = NSMenuItem(); main.addItem(appItem); let app = NSMenu(title: "Sua View"); appItem.submenu = app
        app.addItem(menuItem("Sua View에 관하여", #selector(showAbout(_:)), target: self)); app.addItem(.separator())
        let services = NSMenuItem(title: "서비스", action: nil, keyEquivalent: ""); let servicesMenu = NSMenu(title: "서비스"); services.submenu = servicesMenu; app.addItem(services); NSApp.servicesMenu = servicesMenu
        app.addItem(.separator()); app.addItem(menuItem("Sua View 가리기", #selector(NSApplication.hide(_:)), "h", target: NSApp))
        app.addItem(menuItem("기타 가리기", #selector(NSApplication.hideOtherApplications(_:)), "h", target: NSApp, modifiers: [.command, .option]))
        app.addItem(menuItem("모두 보기", #selector(NSApplication.unhideAllApplications(_:)), target: NSApp)); app.addItem(.separator())
        app.addItem(menuItem("Sua View 종료", #selector(NSApplication.terminate(_:)), "q", target: NSApp))
        let fileItem = NSMenuItem(); main.addItem(fileItem); let file = NSMenu(title: "파일"); fileItem.submenu = file
        file.addItem(menuItem("열기…", #selector(ReaderController.openPanel(_:)), "o"))
        let recentItem = NSMenuItem(title: "최근 열어본 책", action: nil, keyEquivalent: ""); let recent = NSMenu(title: "최근 열어본 책")
        recent.delegate = self; recentItem.submenu = recent; file.addItem(recentItem)
        file.addItem(.separator()); file.addItem(menuItem("책 닫기", #selector(ReaderController.closeBook(_:)), "w"))
        let viewItem = NSMenuItem(); main.addItem(viewItem); let view = NSMenu(title: "보기"); viewItem.submenu = view
        view.addItem(menuItem("두 페이지 보기", #selector(ReaderController.toggleSpread(_:)), "d", modifiers: []))
        view.addItem(menuItem("첫 페이지는 표지 한 장", #selector(ReaderController.toggleCover(_:))))
        view.addItem(menuItem("오른쪽에서 왼쪽으로 읽기", #selector(ReaderController.toggleRTL(_:)), "r", modifiers: []))
        view.addItem(.separator()); view.addItem(menuItem("화면에 맞추기", #selector(ReaderController.fitPage(_:)), "0", modifiers: []))
        view.addItem(menuItem("너비에 맞추기", #selector(ReaderController.fitToWidth(_:)), "w", modifiers: []))
        view.addItem(menuItem("확대", #selector(ReaderController.zoomIn(_:)), "+", modifiers: [])); view.addItem(menuItem("축소", #selector(ReaderController.zoomOut(_:)), "-", modifiers: []))
        view.addItem(.separator()); view.addItem(menuItem("전체 화면", #selector(ReaderController.toggleFullscreen(_:)), "f", modifiers: [.command, .control]))
        let navigateItem = NSMenuItem(); main.addItem(navigateItem); let navigate = NSMenu(title: "이동"); navigateItem.submenu = navigate
        navigate.addItem(menuItem("이전 페이지", #selector(ReaderController.previousPage(_:)), String(UnicodeScalar(NSPageUpFunctionKey)!), modifiers: []))
        navigate.addItem(menuItem("다음 페이지", #selector(ReaderController.nextPage(_:)), " ", modifiers: []))
        navigate.addItem(menuItem("첫 페이지", #selector(ReaderController.firstPage(_:)), String(UnicodeScalar(NSHomeFunctionKey)!), modifiers: []))
        navigate.addItem(menuItem("마지막 페이지", #selector(ReaderController.lastPage(_:)), String(UnicodeScalar(NSEndFunctionKey)!), modifiers: []))
        navigate.addItem(.separator()); navigate.addItem(menuItem("페이지 이동…", #selector(ReaderController.goToPage(_:)), "g", modifiers: []))
        let windowItem = NSMenuItem(); main.addItem(windowItem); let windowMenu = NSMenu(title: "윈도우"); windowItem.submenu = windowMenu
        windowMenu.addItem(menuItem("최소화", #selector(NSWindow.performMiniaturize(_:)), "m", target: reader.window))
        windowMenu.addItem(menuItem("확대/축소", #selector(NSWindow.performZoom(_:)), target: reader.window)); NSApp.windowsMenu = windowMenu
        let helpItem = NSMenuItem(); main.addItem(helpItem); let help = NSMenu(title: "도움말"); helpItem.submenu = help
        help.addItem(menuItem("단축키", #selector(ReaderController.shortcutHelp(_:)), "?", modifiers: [])); NSApp.helpMenu = help
    }
}

extension AppDelegate: NSMenuDelegate {
    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        for url in NSDocumentController.shared.recentDocumentURLs.prefix(12) {
            let item = NSMenuItem(title: url.lastPathComponent, action: #selector(reopenRecent(_:)), keyEquivalent: ""); item.target = self; item.representedObject = url; item.toolTip = url.path; menu.addItem(item)
        }
        if menu.items.isEmpty { let item = NSMenuItem(title: "최근 열어본 책이 없습니다", action: nil, keyEquivalent: ""); item.isEnabled = false; menu.addItem(item) }
        else { menu.addItem(.separator()); let item = NSMenuItem(title: "메뉴 지우기", action: #selector(clearRecent(_:)), keyEquivalent: ""); item.target = self; menu.addItem(item) }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
private let delegate = AppDelegate()
app.delegate = delegate
app.run()
