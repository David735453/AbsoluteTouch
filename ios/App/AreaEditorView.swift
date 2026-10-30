import AbsoluteTouchCore
import SwiftUI

/// Configures the part of the iPad that accepts Pencil input.
///
/// The page is reachable only from the pairing screen. Every control is also a
/// slider or stepper, so the same result is available without the drag gesture
/// and VoiceOver can operate it.
struct AreaEditorView: View {
	/// Saved area, shared with the pairing screen.
	@Binding var settings: ActiveAreaSettings

	/// Shape of the screen canvas, used for both the preview and the ratio math.
	let canvasAspect: Double

	private var model: AreaEditorModel {
		AreaEditorModel(settings: $settings, canvasAspect: canvasAspect)
	}

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: 20) {
				Text(Copy.text(.areaExplanation))
					.font(.callout)
					.foregroundStyle(.secondary)
					.fixedSize(horizontal: false, vertical: true)

				AreaPreview(model: model)

				Text(model.summary)
					.font(.subheadline.monospacedDigit())
					.foregroundStyle(.secondary)

				lockCard(model)

				if model.isLocked {
					ratioSection(model)
				}

				sizeCard(model)
				positionCard(model)

				Button(role: .destructive) {
					model.reset()
				} label: {
					Label(Copy.text(.areaReset), systemImage: "arrow.counterclockwise")
						.frame(maxWidth: .infinity)
				}
				.buttonStyle(.bordered)
				.controlSize(.large)
			}
			.padding(24)
			.frame(maxWidth: 680, alignment: .leading)
			.frame(maxWidth: .infinity)
		}
		.background(Color(uiColor: .systemGroupedBackground))
		.navigationTitle(Copy.text(.drawingArea))
		.navigationBarTitleDisplayMode(.inline)
	}

	private func lockCard(_ model: AreaEditorModel) -> some View {
		Card {
			Toggle(
				isOn: Binding(
					get: { model.isLocked },
					set: { model.setLocked($0) }
				)
			) {
				Text(Copy.text(.areaLockLabel))
					.font(.headline)
			}

			Text(Copy.text(.areaLockHint))
				.font(.footnote)
				.foregroundStyle(.secondary)
				.fixedSize(horizontal: false, vertical: true)
		}
	}

	private func ratioSection(_ model: AreaEditorModel) -> some View {
		Card {
			Text(Copy.text(.areaRatioLabel))
				.font(.headline)

			ScrollView(.horizontal, showsIndicators: false) {
				HStack(spacing: 8) {
					ForEach(AspectRatio.presets, id: \.self) { ratio in
						Button {
							model.setRatio(ratio)
						} label: {
							Text(ratio.label)
						}
						.buttonStyle(.bordered)
						.tint(model.ratio == ratio ? Color.accentColor : Color.secondary)
						.accessibilityAddTraits(
							model.ratio == ratio ? .isSelected : []
						)
					}
				}
				.padding(.vertical, 2)
			}

			DisclosureGroup(Copy.text(.areaCustomRatio)) {
				Stepper(value: model.ratioWidth, in: 1...50) {
					Text(
						Copy.format(
							.areaSideValue,
							Copy.text(.areaRatioWidthLabel),
							model.ratio.width
						)
					)
				}

				Stepper(value: model.ratioHeight, in: 1...50) {
					Text(
						Copy.format(
							.areaSideValue,
							Copy.text(.areaRatioHeightLabel),
							model.ratio.height
						)
					)
				}
			}
			.font(.callout)
		}
	}

	private func sizeCard(_ model: AreaEditorModel) -> some View {
		Card {
			if model.isLocked {
				sliderRow(
					title: Copy.text(.areaSizeLabel),
					value: Copy.format(.areaSizeValue, model.percent(model.resolved.height)),
					binding: model.height,
					range: ActiveArea.minimumDimension...1
				)
			} else {
				sliderRow(
					title: Copy.text(.areaWidthLabel),
					value: Copy.format(.areaWidthValue, model.percent(model.resolved.width)),
					binding: model.width,
					range: ActiveArea.minimumDimension...1
				)

				sliderRow(
					title: Copy.text(.areaHeightLabel),
					value: Copy.format(.areaHeightValue, model.percent(model.resolved.height)),
					binding: model.height,
					range: ActiveArea.minimumDimension...1
				)
			}
		}
	}

	private func positionCard(_ model: AreaEditorModel) -> some View {
		Card {
			Text(Copy.text(.areaPositionLabel))
				.font(.headline)

			if model.resolved.width < 0.999 {
				sliderRow(
					title: Copy.text(.areaHorizontalLabel),
					value: nil,
					binding: model.centerX,
					range: model.resolved.width / 2...(1 - model.resolved.width / 2)
				)
			}

			if model.resolved.height < 0.999 {
				sliderRow(
					title: Copy.text(.areaVerticalLabel),
					value: nil,
					binding: model.centerY,
					range: model.resolved.height / 2...(1 - model.resolved.height / 2)
				)
			}
		}
	}

	private func sliderRow(
		title: String,
		value: String?,
		binding: Binding<Double>,
		range: ClosedRange<Double>
	) -> some View {
		VStack(alignment: .leading, spacing: 6) {
			HStack(alignment: .firstTextBaseline) {
				Text(title)
					.font(.callout)
				Spacer(minLength: 8)
				if let value {
					Text(value)
						.font(.callout.monospacedDigit())
						.foregroundStyle(.secondary)
				}
			}
			Slider(value: binding, in: range)
				.accessibilityLabel(title)
				.accessibilityValue(value ?? "")
		}
	}
}

#Preview {
	NavigationStack {
		AreaEditorView(
			settings: .constant(.default),
			canvasAspect: 4.0 / 3.0
		)
	}
}
