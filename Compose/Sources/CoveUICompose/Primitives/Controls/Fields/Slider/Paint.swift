import Foundation
import CoveUIScene
import CoveUIText
import CoveUIImaging

extension SliderHost {
    func render(node: Node, style: AnySliderStyle, origin: CGPoint, list: DrawList) {
        let appearance = style.resolve(SliderStyleConfiguration(
            value: clampedValue(value.wrappedValue), range: range,
            isPressed: node.attachments[Self.pressedKey] as? Bool ?? false,
            isHovered: node.attachments[Self.hoveredKey] as? Bool ?? false,
            isFocused: FocusChainHolder.current?.focused === node
                && FocusChainHolder.current?.isFocusVisible == true,
            isEnabled: isEnabled, theme: node.theme))
        let geometry = SliderGeometry(node: node, axis: axis)
        guard geometry.length > 0, geometry.breadth > 0 else { return }
        list.addRoundedRect(geometry.rect(at: 0, length: geometry.length,
            thickness: appearance.trackHeight, origin: origin),
            radius: appearance.trackCornerRadius, color: appearance.trackColor)

        let span = range.upperBound - range.lowerBound
        let fraction = span > 0 ? Float((clampedValue(value.wrappedValue) - range.lowerBound) / span) : 0
        let position = geometry.position(for: fraction)
        let fillStart = geometry.isReversed ? position : 0
        let fillLength = geometry.isReversed ? geometry.length - position : position
        if fillLength > 0 {
            list.addRoundedRect(geometry.rect(at: fillStart, length: fillLength,
                thickness: appearance.trackHeight, origin: origin),
                radius: appearance.trackCornerRadius, color: appearance.fillColor)
        }

        if showsTicks, let step, span.isFinite, span > 0 {
            let count = (span / step).rounded(.down)
            // Small 1-px hairlines just outside the track (below it on a
            // horizontal axis, beside it on a vertical one), muted, hidden
            // under the knob, and omitted when they would crowd together.
            let spacing = count > 0 ? geometry.length / Float(count) : 0
            if count.isFinite, count >= 1, count <= 40, spacing >= 10 {
                let tickColor = node.theme.colors.onSurfaceMuted.multipliedAlpha(0.6)
                let thumbHalf = appearance.thumbLength / 2 + 2
                let trackRect = geometry.rect(at: 0, length: geometry.length,
                    thickness: appearance.trackHeight, origin: origin)
                for index in 0...Int(count) {
                    let tick = geometry.position(for: Float(Double(index) * step / span))
                    guard abs(tick - position) > thumbHalf else { continue }
                    if geometry.axis == .horizontal {
                        let x = Float(origin.x) + tick - 0.5
                        list.addRect(UIRect(x: x, y: trackRect.maxY + 2, width: 1, height: 2),
                                     color: tickColor)
                    } else {
                        let y = Float(origin.y) + tick - 0.5
                        list.addRect(UIRect(x: trackRect.maxX + 2, y: y, width: 2, height: 1),
                                     color: tickColor)
                    }
                }
            }
        }

        // The knob is a capsule: longer along the drag axis than across it.
        let breadth = min(appearance.thumbDiameter, geometry.breadth)
        let length = min(appearance.thumbLength, geometry.length)
        let half = length / 2
        let centre = min(max(half, position), geometry.length - half)
        let thumb = geometry.rect(at: centre - half, length: length, thickness: breadth, origin: origin)
        let radius = min(thumb.width, thumb.height) * 0.5
        // A held knob grows slightly, like a magnifying lens over the track.
        if let shadow = appearance.thumbShadow {
            list.addShadow(shadow, rect: thumb, cornerRadius: radius)
        }
        if let border = appearance.thumbBorderColor, appearance.thumbBorderWidth > 0 {
            let width = appearance.thumbBorderWidth
            let outer = UIRect(x: thumb.minX - width, y: thumb.minY - width,
                               width: thumb.width + 2 * width, height: thumb.height + 2 * width)
            list.addRoundedRect(outer, radius: radius + width, color: border)
        }
        list.addRoundedRect(thumb, radius: radius, color: appearance.thumbColor)
    }
}
