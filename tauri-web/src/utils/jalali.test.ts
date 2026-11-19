import { describe, expect, it } from 'vitest';
import {
  formatJalaliDate,
  gregorianToJalali,
  jalaliToGregorian,
  latinDigits,
} from './jalali';

describe('Persian digit normalization', () => {
  it('normalizes Persian and Arabic-Indic digits without changing other text', () => {
    expect(latinDigits('۱۴۰۵/٠٨/۲۹ کالا')).toBe('1405/08/29 کالا');
  });
});

describe('Jalali date conversion', () => {
  it('converts Nowruz 1405 to its Gregorian date', () => {
    expect(gregorianToJalali('2026-03-21')).toBe('1405/01/01');
    expect(jalaliToGregorian('۱۴۰۵/۰۱/۰۱')).toBe('2026-03-21');
  });

  it('accepts slash and dash separators', () => {
    expect(jalaliToGregorian('1405-01-01')).toBe('2026-03-21');
  });

  it('rejects malformed and out-of-range Jalali dates', () => {
    expect(jalaliToGregorian('1405/13/01')).toBeNull();
    expect(jalaliToGregorian('1405/00/01')).toBeNull();
    expect(jalaliToGregorian('not-a-date')).toBeNull();
  });

  it('formats ISO dates using Persian numerals', () => {
    expect(formatJalaliDate('2026-03-21')).toBe('۱۴۰۵/۰۱/۰۱');
  });
});
