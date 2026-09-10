import { describe, it, expect } from 'vitest';
import { formatNumber, formatGb, formatMb, formatKb, formatBp } from './utils';

describe('utils', () => {
  it('formats numbers correctly', () => {
    expect(formatNumber(1234567)).toBe('1,234,567');
  });

  it('formats Gb correctly', () => {
    expect(formatGb(3000000000)).toBe('3.00');
    expect(formatGb(154321098)).toBe('0.15');
  });

  it('formats Mb correctly', () => {
    expect(formatMb(1234567)).toBe('1.23');
  });

  it('formats Kb correctly', () => {
    expect(formatKb(1234)).toBe('1.2');
  });

  it('formats Bp correctly', () => {
    expect(formatBp(123)).toBe('123');
  });
});
