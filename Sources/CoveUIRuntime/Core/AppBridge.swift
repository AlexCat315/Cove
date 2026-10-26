import CovePlatformCore

public final class PlatformEventBridge: @unchecked Sendable {
    public struct SubscriptionToken: Hashable, Sendable {
        let raw: UInt64
    }

    private var subscribers: [SubscriptionToken: (InputEvent) -> Void] = [:]
    private var nextSubscriberID: UInt64 = 0

    public init() {}

    @discardableResult
    public func subscribe(_ handler: @escaping (InputEvent) -> Void) -> SubscriptionToken {
        nextSubscriberID &+= 1
        let token = SubscriptionToken(raw: nextSubscriberID)
        subscribers[token] = handler
        return token
    }

    public func unsubscribe(_ token: SubscriptionToken) {
        subscribers.removeValue(forKey: token)
    }

    public func publish(_ event: InputEvent) {
        for handler in subscribers.values {
            handler(event)
        }
    }
}
