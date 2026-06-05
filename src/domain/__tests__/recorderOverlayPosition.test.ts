import { describe, expect, it } from 'vitest';
import { calculateRecorderOverlayPosition } from '../recorderOverlayPosition';

describe('recorder overlay position', () => {
  it('centers the overlay above the bottom edge of the primary monitor work area', () => {
    expect(
      calculateRecorderOverlayPosition({
        workArea: {
          position: { x: 0, y: 0 },
          size: { width: 1920, height: 1040 },
        },
        scaleFactor: 1,
        overlaySize: { width: 256, height: 52 },
        marginBottom: 18,
      }),
    ).toEqual({ x: 832, y: 970 });
  });

  it('keeps negative monitor origins when positioning on a left-side secondary monitor', () => {
    expect(
      calculateRecorderOverlayPosition({
        workArea: {
          position: { x: -1920, y: 0 },
          size: { width: 1920, height: 1040 },
        },
        scaleFactor: 1,
        overlaySize: { width: 256, height: 52 },
        marginBottom: 18,
      }),
    ).toEqual({ x: -1088, y: 970 });
  });

  it('uses physical pixels for mixed-DPI monitors', () => {
    expect(
      calculateRecorderOverlayPosition({
        workArea: {
          position: { x: 1920, y: 0 },
          size: { width: 3840, height: 2080 },
        },
        scaleFactor: 2,
        overlaySize: { width: 256, height: 52 },
        marginBottom: 18,
      }),
    ).toEqual({ x: 3584, y: 1940 });
  });
});
