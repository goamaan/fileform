import AVFoundation
import Foundation
import Observation
import FileformDomain

/// Playback only. The public runtime must provide a local, verified zero-origin
/// file (and retain its preview lease); this controller does not normalize media.
@MainActor @Observable
final class MediaPlaybackController {
    private(set) var player: AVPlayer?
    private(set) var isPlaying = false
    private(set) var isLoading = false
    private(set) var isSeeking = false
    private(set) var playhead = MediaTime(ticks: 0, timescale: 1000)
    private(set) var duration: MediaTime?
    private(set) var selection: MediaInterval?
    private(set) var selectionOnly = true
    var error: String? { playbackError ?? selectionError }

    private var playbackError: String?
    private var selectionError: String?
    @ObservationIgnored private var resources: PlaybackResources?
    @ObservationIgnored private var loadingAsset: AVURLAsset?
    @ObservationIgnored private var readyTimeout: Task<Void, Never>?
    @ObservationIgnored private var loadID = UUID()
    @ObservationIgnored private var seekID = UUID()
    @ObservationIgnored private var boundaryID = UUID()
    @ObservationIgnored private var wantsToPlay = false

    /// Loading never starts playback. Read `isLoading` until the player item is
    /// ready; `isSeeking` also covers its initial positioning at the range start.
    func load(url: URL, duration: MediaTime, selection: MediaInterval) async {
        teardown()
        let id = loadID
        isLoading = true
        var ownsScope = false
        var transferredScope = false
        defer { if ownsScope && !transferredScope { url.stopAccessingSecurityScopedResource() } }
        do {
            guard url.isFileURL else { throw PlaybackFailure("Playback requires a verified local file.") }
            _ = try MediaPlaybackBounds(duration: duration, selection: selection, selectionOnly: selectionOnly)
            self.duration = duration
            self.selection = selection
            ownsScope = url.startAccessingSecurityScopedResource()
            let asset = AVURLAsset(url: url)
            loadingAsset = asset
            let (playable, actualDuration) = try await asset.load(.isPlayable, .duration)
            try Task.checkCancellation()
            guard id == loadID else { return }
            guard playable, actualDuration.isNumeric, actualDuration.epoch == 0,
                  CMTimeCompare(actualDuration, duration.playbackCMTime) >= 0 else {
                throw PlaybackFailure("The playback file is unsupported or shorter than the measured recording. Prepare a new playback preview.")
            }
            loadingAsset = nil
            let item = AVPlayerItem(asset: asset)
            let player = AVPlayer(playerItem: item)
            player.actionAtItemEnd = .pause
            self.player = player
            let resources = PlaybackResources(player: player, scopedURL: ownsScope ? url : nil)
            self.resources = resources
            transferredScope = true
            installObservers(player: player, item: item, resources: resources, loadID: id)
            installBoundary()
            readyTimeout = Task { @MainActor [weak self] in
                do { try await Task.sleep(for: .seconds(15)) } catch { return }
                guard let self, self.loadID == id, self.isLoading else { return }
                self.failPlayback("The playback preview did not become ready. Try loading it again.")
            }
        } catch {
            guard id == loadID else { return }
            loadingAsset = nil
            isLoading = false
            if error is CancellationError || Task.isCancelled {
                teardown()
            } else {
                playbackError = error.localizedDescription
            }
        }
    }

    /// Call for a committed numeric edit or each handle movement. Editing pauses
    /// playback and supersedes pending seeks, so an old completion cannot play.
    @discardableResult
    func updateSelection(_ value: MediaInterval) -> Bool {
        pause()
        guard let duration else { selection = value; return false }
        do {
            let bounds = try MediaPlaybackBounds(duration: duration, selection: value, selectionOnly: selectionOnly)
            selection = value
            selectionError = nil
            installBoundary()
            if let time = try? bounds.clamped(playhead), time != playhead { seek(to: time) }
            return true
        } catch {
            selectionError = error.localizedDescription
            // Retain the last valid draft for display, but don't permit a native
            // VideoPlayer transport control to play an obsolete range.
            resources?.player.currentItem?.forwardPlaybackEndTime = .zero
            removeBoundary()
            return false
        }
    }

    func setSelectionOnly(_ value: Bool) {
        guard value != selectionOnly else { return }
        pause()
        selectionOnly = value
        guard selectionError == nil else { return }
        installBoundary()
        if let bounds, let target = try? bounds.clamped(playhead), target != playhead { seek(to: target) }
    }

    func play() {
        guard let player, player.currentItem?.status == .readyToPlay,
              !isLoading, selectionError == nil, let bounds else { return }
        let current = MediaTime(playbackTime: player.currentTime()) ?? playhead
        seek(to: bounds.startForPlayback(at: current), resume: true)
    }

    func pause() {
        wantsToPlay = false
        seekID = UUID()
        player?.pause()
        player?.currentItem?.cancelPendingSeeks()
        isPlaying = false
        isSeeking = false
    }

    func togglePlayback() {
        if wantsToPlay || isPlaying { pause() } else { play() }
    }

    /// Seek is asynchronous at AVFoundation's boundary. A newer seek, pause,
    /// selection change or load invalidates this completion before it can play.
    func seek(to time: MediaTime, resume: Bool = false) {
        guard let player, player.currentItem?.status == .readyToPlay,
              selectionError == nil, let bounds else { return }
        let target: MediaTime
        do { target = try bounds.clamped(time) }
        catch { playbackError = error.localizedDescription; pause(); return }
        pause()
        playbackError = nil
        isSeeking = true
        wantsToPlay = resume
        let seek = UUID(); seekID = seek
        let load = loadID
        player.seek(to: target.playbackCMTime, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
            Task { @MainActor [weak self] in
                guard let self, self.loadID == load, self.seekID == seek,
                      let player = self.player else { return }
                self.isSeeking = false
                guard finished else {
                    self.wantsToPlay = false
                    self.playbackError = "The playback preview could not seek to that time. Try again."
                    return
                }
                self.playhead = MediaTime(playbackTime: player.currentTime()) ?? target
                if resume, self.wantsToPlay, self.selectionError == nil,
                   let active = self.bounds,
                   CMTimeCompare(self.playhead.playbackCMTime, active.upper.playbackCMTime) < 0 {
                    player.play()
                } else { self.wantsToPlay = false }
            }
        }
    }

    /// Call before releasing/replacing the runtime's playback-preview lease.
    func teardown() {
        loadID = UUID(); seekID = UUID(); boundaryID = UUID()
        readyTimeout?.cancel(); readyTimeout = nil
        loadingAsset?.cancelLoading(); loadingAsset = nil
        resources?.invalidate(); resources = nil
        player = nil
        duration = nil; selection = nil
        playhead = .init(ticks: 0, timescale: 1000)
        isPlaying = false; isLoading = false; isSeeking = false; wantsToPlay = false
        playbackError = nil; selectionError = nil
    }

    private var bounds: MediaPlaybackBounds? {
        guard let duration, let selection else { return nil }
        return try? .init(duration: duration, selection: selection, selectionOnly: selectionOnly)
    }

    private func installObservers(player: AVPlayer, item: AVPlayerItem, resources: PlaybackResources, loadID id: UUID) {
        resources.observations.append(item.observe(\.status, options: [.initial, .new]) { [weak self] item, _ in
            let status = item.status.rawValue
            let message = item.error?.localizedDescription
            Task { @MainActor [weak self] in
                guard let self, self.loadID == id else { return }
                switch AVPlayerItem.Status(rawValue: status) {
                case .readyToPlay:
                    self.readyTimeout?.cancel(); self.readyTimeout = nil
                    let wasLoading = self.isLoading
                    self.isLoading = false
                    if wasLoading, let lower = self.bounds?.lower { self.seek(to: lower) }
                case .failed:
                    self.failPlayback(message ?? "This playback preview could not be opened.")
                default: break
                }
            }
        })
        resources.observations.append(player.observe(\.timeControlStatus, options: [.initial, .new]) { [weak self] _, _ in
            Task { @MainActor [weak self] in
                guard let self, self.loadID == id else { return }
                self.transportChanged()
            }
        })
        resources.periodic = player.addPeriodicTimeObserver(forInterval: CMTime(value: 1, timescale: 30), queue: .main) { [weak self] time in
            MainActor.assumeIsolated {
                guard let self, self.loadID == id, !self.isSeeking,
                      let value = MediaTime(playbackTime: time) else { return }
                self.playhead = self.bounds.flatMap { try? $0.clamped(value) } ?? value
            }
        }
        let center = NotificationCenter.default
        resources.notifications.append(center.addObserver(forName: AVPlayerItem.didPlayToEndTimeNotification, object: item, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.loadID == id else { return }
                self.reachedEnd()
            }
        })
        resources.notifications.append(center.addObserver(forName: AVPlayerItem.failedToPlayToEndTimeNotification, object: item, queue: .main) { [weak self] notification in
            let message = (notification.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? Error)?.localizedDescription
            MainActor.assumeIsolated {
                guard let self, self.loadID == id else { return }
                self.failPlayback(message ?? "Playback stopped because the preview could not be read.")
            }
        })
        resources.notifications.append(center.addObserver(forName: AVPlayerItem.timeJumpedNotification, object: item, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.loadID == id else { return }
                self.reconcileExternalSeek()
            }
        })
    }

    private func installBoundary() {
        removeBoundary()
        guard selectionError == nil, let resources, let bounds else { return }
        resources.player.currentItem?.forwardPlaybackEndTime = bounds.upper.playbackCMTime
        resources.player.currentItem?.reversePlaybackEndTime = bounds.lower.playbackCMTime
        let id = boundaryID
        let load = loadID
        resources.boundary = resources.player.addBoundaryTimeObserver(forTimes: [NSValue(time: bounds.upper.playbackCMTime)], queue: .main) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.loadID == load, self.boundaryID == id else { return }
                self.reachedEnd()
            }
        }
    }

    private func removeBoundary() {
        boundaryID = UUID()
        guard let resources, let token = resources.boundary else { return }
        resources.player.removeTimeObserver(token)
        resources.boundary = nil
    }

    private func reachedEnd() {
        pause()
        if let upper = bounds?.upper { playhead = upper }
    }

    private func transportChanged() {
        guard let player else { return }
        if selectionError != nil || isLoading {
            player.pause(); isPlaying = false
            return
        }
        isPlaying = player.timeControlStatus == .playing
        if player.timeControlStatus == .paused, !isSeeking { wantsToPlay = false }
        if isPlaying { reconcileExternalSeek() }
    }

    private func reconcileExternalSeek() {
        guard !isSeeking, let player, selectionError == nil, let bounds,
              let current = MediaTime(playbackTime: player.currentTime()),
              let clamped = try? bounds.clamped(current) else { return }
        if CMTimeCompare(current.playbackCMTime, clamped.playbackCMTime) != 0 {
            let resume = player.timeControlStatus != .paused && CMTimeCompare(current.playbackCMTime, bounds.lower.playbackCMTime) < 0
            seek(to: clamped, resume: resume)
        } else { playhead = current }
    }

    private func failPlayback(_ message: String) {
        pause()
        readyTimeout?.cancel(); readyTimeout = nil
        isLoading = false
        playbackError = message
    }
}

/// Pure rational range policy, separated so tests can exercise seeking/replay
/// boundaries without pretending that a mock proves AVPlayer timing behavior.
struct MediaPlaybackBounds {
    let lower: MediaTime
    let upper: MediaTime
    init(duration: MediaTime, selection: MediaInterval, selectionOnly: Bool) throws {
        try duration.validate()
        guard duration.ticks > 0 else { throw PlaybackFailure("Playback needs a positive measured duration.") }
        try selection.validate(duration: duration)
        lower = selectionOnly ? selection.start : .init(ticks: 0, timescale: 1000)
        upper = selectionOnly ? selection.end : duration
    }
    func clamped(_ time: MediaTime) throws -> MediaTime {
        try time.validate()
        if CMTimeCompare(time.playbackCMTime, lower.playbackCMTime) < 0 { return lower }
        if CMTimeCompare(time.playbackCMTime, upper.playbackCMTime) > 0 { return upper }
        return time
    }
    func startForPlayback(at time: MediaTime) -> MediaTime {
        guard time.ticks >= 0, time.timescale > 0,
              CMTimeCompare(time.playbackCMTime, lower.playbackCMTime) >= 0,
              CMTimeCompare(time.playbackCMTime, upper.playbackCMTime) < 0 else { return lower }
        return time
    }
}

private extension MediaTime {
    var playbackCMTime: CMTime { CMTime(value: ticks, timescale: timescale) }
    init?(playbackTime: CMTime) {
        guard playbackTime.isNumeric, playbackTime.epoch == 0,
              playbackTime.value >= 0, playbackTime.timescale > 0 else { return nil }
        self.init(ticks: playbackTime.value, timescale: playbackTime.timescale)
    }
}
private struct PlaybackFailure: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

/// Only the controller mutates this owner, on the main actor. Its final cleanup
/// is transferred to the main queue if ARC releases the controller elsewhere.
private final class PlaybackResources {
    let player: AVPlayer
    var periodic: Any?
    var boundary: Any?
    var observations: [NSKeyValueObservation] = []
    var notifications: [NSObjectProtocol] = []
    var scopedURL: URL?
    init(player: AVPlayer, scopedURL: URL?) { self.player = player; self.scopedURL = scopedURL }

    @MainActor func invalidate() {
        let cleanup = takeCleanup()
        cleanup.perform()
    }
    private func takeCleanup() -> PlaybackCleanup {
        let value = PlaybackCleanup(player: player, periodic: periodic, boundary: boundary,
                                    observations: observations, notifications: notifications, scopedURL: scopedURL)
        periodic = nil; boundary = nil; observations = []; notifications = []; scopedURL = nil
        return value
    }
    deinit {
        let cleanup = takeCleanup()
        if Thread.isMainThread { MainActor.assumeIsolated { cleanup.perform() } }
        else { DispatchQueue.main.async { cleanup.perform() } }
    }
}

/// Tokens are opaque and not Sendable, but cross the queue exactly once for
/// disposal; no callback or background task reads or mutates these fields.
private struct PlaybackCleanup: @unchecked Sendable {
    let player: AVPlayer
    let periodic: Any?
    let boundary: Any?
    let observations: [NSKeyValueObservation]
    let notifications: [NSObjectProtocol]
    let scopedURL: URL?
    @MainActor func perform() {
        player.pause()
        player.currentItem?.cancelPendingSeeks()
        if let periodic { player.removeTimeObserver(periodic) }
        if let boundary { player.removeTimeObserver(boundary) }
        observations.forEach { $0.invalidate() }
        notifications.forEach { NotificationCenter.default.removeObserver($0) }
        player.replaceCurrentItem(with: nil)
        scopedURL?.stopAccessingSecurityScopedResource()
    }
}
