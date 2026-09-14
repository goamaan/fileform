import Foundation
import Observation
import FileformDomain

@MainActor @Observable final class WorkspacePreferences {
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored var networkAccessChanged: ((Bool) -> Void)?
    @ObservationIgnored var destinationRetentionChanged: (() -> Void)?
    var defaultCollision: CollisionPolicy {
        didSet { defaults.set(defaultCollision.rawValue, forKey: "saving.defaultCollision") }
    }
    var rememberOutputFolder: Bool {
        didSet {
            defaults.set(rememberOutputFolder, forKey: "saving.rememberOutputFolder")
            destinationRetentionChanged?()
        }
    }
    var allowLinks: Bool {
        didSet {
            defaults.set(allowLinks, forKey: "network.allowLinks")
            networkAccessChanged?(allowLinks)
        }
    }
    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        defaultCollision = defaults.string(forKey: "saving.defaultCollision").flatMap(CollisionPolicy.init(rawValue:)) ?? .rename
        rememberOutputFolder = defaults.object(forKey: "saving.rememberOutputFolder") as? Bool ?? true
        allowLinks = defaults.object(forKey: "network.allowLinks") as? Bool ?? true
    }
}
