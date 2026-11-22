/** Persian/Jalali date helpers shared by Nabz pages and forms. */

export function latinDigits(value: string): string {
  return value
    .replace(/[۰-۹]/g, digit => String('۰۱۲۳۴۵۶۷۸۹'.indexOf(digit)))
    .replace(/[٠-٩]/g, digit => String('٠١٢٣٤٥٦٧٨٩'.indexOf(digit)));
}

export function jalaliParts(iso: string): { year: number; month: number; day: number } {
  if (!iso) return { year: 1405, month: 1, day: 1 };
  const date = new Date(iso + 'T12:00:00');
  const parts = new Intl.DateTimeFormat('en-u-ca-persian', {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
  }).formatToParts(date);

  return {
    year: Number(parts.find(part => part.type === 'year')?.value),
    month: Number(parts.find(part => part.type === 'month')?.value),
    day: Number(parts.find(part => part.type === 'day')?.value),
  };
}

export function formatJalaliDate(iso: string): string {
  const day = iso?.slice(0, 10) || '';
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) return iso || '—';

  return gregorianToJalali(day)
    .split('/')
    .map((value, index) => new Intl.NumberFormat('fa-IR', {
      useGrouping: false,
      minimumIntegerDigits: index === 0 ? 4 : 2,
    }).format(Number(value)))
    .join('/');
}

export function gregorianToJalali(iso: string): string {
  if (!iso) return '';
  const parts = jalaliParts(iso);
  const digits = new Intl.NumberFormat('fa-IR', { useGrouping: false });
  return [parts.year, parts.month, parts.day].map(value => digits.format(value).padStart(2, '۰')).join('/');
}

export function jalaliToGregorian(raw: string): string | null {
  const match = latinDigits(raw).trim().match(/^(\d{4})[\/-](\d{1,2})[\/-](\d{1,2})$/);
  if (!match) return null;

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;

  const start = Date.UTC(year + 621, 0, 1);
  for (let offset = 0; offset < 460; offset++) {
    const date = new Date(start + offset * 86400000);
    const parts = jalaliParts(date.toISOString().slice(0, 10));
    if (parts.year === year && parts.month === month && parts.day === day) {
      return date.toISOString().slice(0, 10);
    }
  }
  return null;
}

export const jalaliMonths = [
  'فروردین', 'اردیبهشت', 'خرداد', 'تیر', 'مرداد', 'شهریور',
  'مهر', 'آبان', 'آذر', 'دی', 'بهمن', 'اسفند',
];

export const jalaliWeekdays = ['ش', 'ی', 'د', 'س', 'چ', 'پ', 'ج'];
