// M6B NFR-005 真实进程外部录制器（60 Hz 口径）。
//
// 前提：release Qurner.app 已启动、已打开合成长章书（分页模式）且位于章节开头。
// 用法：
//   swiftc -O -parse-as-library scripts/m6b_turnpage_recorder.swift -o /tmp/m6b-recorder
//   /tmp/m6b-recorder [--out DIR] [--samples 20] [--scale 2]
//
// 输出：DIR/turnpage-60fps.mov、turnpage-samples.csv、turnpage-summary.json、key-times.txt。
// 仅分析窗口“正文区域”（排除顶部标题/进度/工具栏），键盘交替左右方向键避免越界。

import AppKit
import AVFoundation
import CoreGraphics
import CoreMedia
import ScreenCaptureKit
import Foundation

func log(_ message: String) { FileHandle.standardError.write((message + "\n").data(using: .utf8)!) }

let args = CommandLine.arguments
func argValue(_ name: String, _ fallback: String) -> String {
    if let index = args.firstIndex(of: name), index + 1 < args.count { return args[index + 1] }
    return fallback
}
let outDir = argValue("--out", NSTemporaryDirectory() + "qurner-m6b/turnpage")
let samplesWanted = Int(argValue("--samples", "20")) ?? 20
let frameInterval = CMTime(value: 1, timescale: 60)

struct FrameRecord { let pts: Double; let grid: [UInt8] }
struct KeyRecord { let index: Int; let start: Double; let end: Double }

final class Recorder: NSObject, SCStreamOutput {
    let window: SCWindow
    let scale: CGFloat
    let outDir: String
    let samplesWanted: Int
    let videoURL: URL
    let csvURL: URL
    let summaryURL: URL
    let keyFile: String
    let gridW = 40
    let gridH = 30
    var stream: SCStream?
    var writer: AVAssetWriter?
    var input: AVAssetWriterInput?
    var adaptor: AVAssetWriterInputPixelBufferAdaptor?
    var started = false
    var firstPts = 0.0
    var lastWriteTime = 0.0
    var pixelWidth = 0
    var pixelHeight = 0
    var contentTop = 0
    var contentBottom = 0
    var contentLeft = 0
    var contentRight = 0
    var frames: [FrameRecord] = []

    init(window: SCWindow, outDir: String, samplesWanted: Int) {
        self.window = window
        self.outDir = outDir
        self.samplesWanted = samplesWanted
        self.scale = NSScreen.main?.backingScaleFactor ?? 2
        self.videoURL = URL(fileURLWithPath: outDir + "/turnpage-60fps.mov")
        self.csvURL = URL(fileURLWithPath: outDir + "/turnpage-samples.csv")
        self.summaryURL = URL(fileURLWithPath: outDir + "/turnpage-summary.json")
        self.keyFile = outDir + "/key-times.txt"
        super.init()
        self.pixelWidth = Int(window.frame.width * self.scale)
        self.pixelHeight = Int(window.frame.height * self.scale)
        // 正文区域：排除顶部 64pt（标题/进度/工具栏）与底部 24pt，左右各 20pt。
        self.contentTop = Int(64 * self.scale)
        self.contentBottom = self.pixelHeight - Int(24 * self.scale)
        self.contentLeft = Int(20 * self.scale)
        self.contentRight = self.pixelWidth - Int(20 * self.scale)
    }

    func start() async throws {
        try FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
        try? FileManager.default.removeItem(at: videoURL)
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let config = SCStreamConfiguration()
        config.width = pixelWidth
        config.height = pixelHeight
        config.minimumFrameInterval = frameInterval
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.showsCursor = false
        config.queueDepth = 6
        let writer = try AVAssetWriter(outputURL: videoURL, fileType: .mov)
        let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: pixelWidth,
            AVVideoHeightKey: pixelHeight,
        ])
        input.expectsMediaDataInRealTime = true
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: pixelWidth,
            kCVPixelBufferHeightKey as String: pixelHeight,
        ])
        writer.add(input)
        self.writer = writer
        self.input = input
        self.adaptor = adaptor
        let stream = SCStream(filter: filter, configuration: config, delegate: nil)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: DispatchQueue(label: "screen"))
        self.stream = stream
        try await stream.startCapture()
        log("capture \(pixelWidth)x\(pixelHeight) content=[\(contentLeft),\(contentTop)]-[\(contentRight),\(contentBottom)]")
    }

    func grid(_ buffer: CVPixelBuffer) -> [UInt8] {
        CVPixelBufferLockBaseAddress(buffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
        let rowBytes = CVPixelBufferGetBytesPerRow(buffer)
        guard let base = CVPixelBufferGetBaseAddress(buffer) else { return [] }
        let bytes = base.assumingMemoryBound(to: UInt8.self)
        var result = [UInt8](repeating: 0, count: gridW * gridH)
        let regionW = max(1, contentRight - contentLeft)
        let regionH = max(1, contentBottom - contentTop)
        let cellW = max(1, regionW / gridW)
        let cellH = max(1, regionH / gridH)
        for gy in 0..<gridH {
            for gx in 0..<gridW {
                var total = 0
                var count = 0
                var y = contentTop + gy * cellH
                while y < min(contentTop + (gy + 1) * cellH, contentBottom) {
                    var x = contentLeft + gx * cellW
                    while x < min(contentLeft + (gx + 1) * cellW, contentRight) {
                        let offset = y * rowBytes + x * 4
                        total += Int(bytes[offset]) + Int(bytes[offset + 1]) + Int(bytes[offset + 2])
                        count += 1
                        x += 2
                    }
                    y += 2
                }
                result[gy * gridW + gx] = UInt8(min(255, total / max(1, count) / 3))
            }
        }
        return result
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let pts = CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sampleBuffer))
        if firstPts == 0 { firstPts = pts }
        frames.append(FrameRecord(pts: pts, grid: grid(imageBuffer)))
        if !started, let writer, writer.status == .unknown, writer.startWriting() {
            started = true
            writer.startSession(atSourceTime: CMTime(seconds: pts, preferredTimescale: 600))
        }
        if started, let adaptor, let input, input.isReadyForMoreMediaData {
            adaptor.append(imageBuffer, withPresentationTime: CMTime(seconds: pts - firstPts, preferredTimescale: 600))
            lastWriteTime = pts - firstPts
        }
    }

    func driveKeys() async throws {
        let scriptPath = outDir + "/keypress.applescript"
        let script = """
        set outPath to "\(keyFile)"
        set f to open for access (POSIX file outPath) with write permission
        set eof f to 0
        repeat with i from 1 to \(samplesWanted)
            tell application "Qurner" to activate
            delay 0.05
            set t0 to do shell script "python3 -c 'import time; print(time.time())'"
            if (i mod 2) is 1 then
                tell application "System Events" to key code 124
            else
                tell application "System Events" to key code 123
            end if
            set t1 to do shell script "python3 -c 'import time; print(time.time())'"
            write ((i as text) & "," & t0 & "," & t1 & "\\n") to f
            delay 0.75
        end repeat
        close access f
        """
        try script.write(toFile: scriptPath, atomically: true, encoding: .utf8)
        if let app = NSRunningApplication.runningApplications(withBundleIdentifier: "com.qurner.reader").first {
            _ = app.activate(options: [.activateAllWindows])
        }
        try await Task.sleep(nanoseconds: 2_000_000_000)
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = [scriptPath]
        try process.run()
        process.waitUntilExit()
        try await Task.sleep(nanoseconds: 800_000_000)
    }

    func stop() async {
        if let stream { try? await stream.stopCapture() }
        input?.markAsFinished()
        if let writer, writer.status == .writing {
            writer.endSession(atSourceTime: CMTime(seconds: lastWriteTime, preferredTimescale: 600))
            await writer.finishWriting()
        }
        log("writer status=\(writer?.status.rawValue ?? -1)")
    }

    func analyze() -> Bool {
        func diff(_ a: [UInt8], _ b: [UInt8]) -> Double {
            let n = min(a.count, b.count)
            guard n > 0 else { return 0 }
            var changed = 0
            for i in 0..<n where abs(Int(a[i]) - Int(b[i])) > 8 { changed += 1 }
            return Double(changed) / Double(n)
        }
        let bootEpoch = Date().timeIntervalSince1970 - CACurrentMediaTime()
        var keyLines: [(Int, Double, Double)] = []
        if let content = try? String(contentsOfFile: keyFile, encoding: .utf8) {
            for line in content.split(separator: "\n") {
                let parts = line.split(separator: ",")
                if parts.count == 3, let i = Int(parts[0]), let t0 = Double(parts[1]), let t1 = Double(parts[2]) {
                    keyLines.append((i, t0 - bootEpoch, t1 - bootEpoch))
                }
            }
        }
        var samples: [(Int, Double, Double)] = []
        for key in keyLines.sorted(by: { $0.0 < $1.0 }) {
            let before = frames.last(where: { $0.pts < key.1 })
            guard let baseline = before,
                  let after = frames.first(where: { $0.pts > key.2 && diff($0.grid, baseline.grid) > 0.015 }) else {
                log("sample \(key.0) no new content frame")
                continue
            }
            let observed = (after.pts - key.1) * 1000
            samples.append((key.0, observed, observed + 16.7))
        }
        var csv = "index,key_start_host_s,key_end_host_s,render_host_s,observed_ms,conservative_ms\n"
        for item in samples {
            guard let key = keyLines.first(where: { $0.0 == item.0 }) else { continue }
            csv += "\(item.0),\(String(format: "%.6f", key.1)),\(String(format: "%.6f", key.2)),\(String(format: "%.6f", key.1 + item.1 / 1000)),\(String(format: "%.2f", item.1)),\(String(format: "%.2f", item.2))\n"
        }
        try? csv.write(to: csvURL, atomically: true, encoding: .utf8)
        let conservative = samples.map { $0.2 }.sorted()
        func pct(_ p: Double) -> Double {
            guard !conservative.isEmpty else { return 0 }
            let rank = Int(ceil(p / 100.0 * Double(conservative.count)))
            return conservative[max(0, min(conservative.count - 1, rank - 1))]
        }
        let intervals = zip(frames.dropFirst(), frames).map { $0.pts - $1.pts }
        let fps = intervals.isEmpty ? 0 : 1.0 / intervals.sorted()[intervals.count / 2]
        let maxGap = (intervals.max() ?? 0) * 1000
        let p95 = pct(95)
        let pass = samples.count == samplesWanted && p95 <= 150 && fps >= 55 && maxGap <= 100
        let summary: [String: Any] = [
            "samples": samples.count,
            "fps": fps,
            "max_gap_ms": maxGap,
            "p50_conservative_ms": pct(50),
            "p95_conservative_ms": p95,
            "threshold_ms": 150,
            "content_region": [contentLeft, contentTop, contentRight, contentBottom],
            "calibration": "AppleScript epoch -> host via bootEpoch; frame CMSampleBuffer PTS (host clock)",
            "pass": pass,
        ]
        if let data = try? JSONSerialization.data(withJSONObject: summary, options: [.prettyPrinted, .sortedKeys]) {
            try? data.write(to: summaryURL)
            print(String(data: data, encoding: .utf8)!)
        }
        return pass
    }
}

@main
struct Main {
    static func main() async {
        let content: SCShareableContent
        do { content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true) }
        catch { log("shareable content failed \(error)"); exit(2) }
        let candidates = content.windows.filter { window in
            window.owningApplication?.bundleIdentifier == "com.qurner.reader"
                && window.title == "Qurner"
                && window.frame.width >= 800
                && window.frame.height >= 500
        }
        guard let window = candidates.max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }) else {
            let sizes = content.windows
                .filter { $0.owningApplication?.bundleIdentifier == "com.qurner.reader" }
                .map { "\($0.title)/\(Int($0.frame.width))x\(Int($0.frame.height))" }
            log("no Qurner main window >=800x500; candidates: \(sizes)")
            exit(2)
        }
        log("selected window title=\(window.title) size=\(Int(window.frame.width))x\(Int(window.frame.height))")
        let recorder = Recorder(window: window, outDir: outDir, samplesWanted: samplesWanted)
        do { try await recorder.start() } catch { log("start failed \(error)"); exit(3) }
        do { try await recorder.driveKeys() } catch { log("keys failed \(error)"); exit(3) }
        await recorder.stop()
        exit(recorder.analyze() ? 0 : 1)
    }
}
