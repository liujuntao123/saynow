export interface PhysicalRect {
  position: {
    x: number;
    y: number;
  };
  size: {
    width: number;
    height: number;
  };
}

export interface OverlayPositionInput {
  workArea: PhysicalRect;
  scaleFactor: number;
  overlaySize: {
    width: number;
    height: number;
  };
  marginBottom: number;
}

export interface OverlayPosition {
  x: number;
  y: number;
}

export function calculateRecorderOverlayPosition(input: OverlayPositionInput): OverlayPosition {
  const scaleFactor = input.scaleFactor > 0 ? input.scaleFactor : 1;
  const overlayWidth = input.overlaySize.width * scaleFactor;
  const overlayHeight = input.overlaySize.height * scaleFactor;
  const marginBottom = input.marginBottom * scaleFactor;
  const { position, size } = input.workArea;

  return {
    x: Math.round(position.x + (size.width - overlayWidth) / 2),
    y: Math.round(position.y + size.height - overlayHeight - marginBottom),
  };
}
