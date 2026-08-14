import { describe, it, expect } from 'vitest';
import {
  FIRST_START_TOUR_STEPS,
  computeCalloutPosition,
  computeSpotlightHole,
  clamp,
} from './firstStartTour.js';

describe('firstStartTour', () => {
  it('defines five steps with unique ids and selectors', () => {
    expect(FIRST_START_TOUR_STEPS).toHaveLength(5);
    const ids = FIRST_START_TOUR_STEPS.map((s) => s.id);
    expect(new Set(ids).size).toBe(5);
    for (const step of FIRST_START_TOUR_STEPS) {
      expect(step.selector).toMatch(/^\[data-tour=/);
      expect(step.title.length).toBeGreaterThan(0);
      expect(step.body.length).toBeGreaterThan(0);
    }
  });

  it('clamp keeps values in range', () => {
    expect(clamp(5, 0, 10)).toBe(5);
    expect(clamp(-1, 0, 10)).toBe(0);
    expect(clamp(99, 0, 10)).toBe(10);
  });

  it('computeSpotlightHole pads the target rect', () => {
    const hole = computeSpotlightHole(
      { left: 100, top: 50, width: 200, height: 80 },
      { width: 1200, height: 800 },
    );
    expect(hole.x).toBe(94);
    expect(hole.y).toBe(44);
    expect(hole.width).toBe(212);
    expect(hole.height).toBe(92);
  });

  it('computeCalloutPosition prefers below the target', () => {
    const pos = computeCalloutPosition(
      { left: 100, top: 100, width: 200, height: 40, right: 300, bottom: 140 },
      { width: 1200, height: 800 },
      { width: 320, height: 180 },
    );
    expect(pos.top).toBeGreaterThanOrEqual(140);
    expect(pos.left).toBeGreaterThanOrEqual(12);
    expect(pos.width).toBe(320);
  });
});
