/** @vitest-environment jsdom */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { PageHeading, PrimaryButton } from './PagePrimitives';

describe('PageHeading', () => {
  it('renders its title, subtitle, default eyebrow and action slot', () => {
    render(
      <PageHeading title="کالاها" subtitle="مدیریت موجودی">
        <button>افزودن کالا</button>
      </PageHeading>,
    );
    expect(screen.getByRole('heading', { name: 'کالاها' })).toBeTruthy();
    expect(screen.getByText('مدیریت موجودی')).toBeTruthy();
    expect(screen.getByText('مدیریت فروشگاه')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'افزودن کالا' })).toBeTruthy();
  });

  it('uses the supplied eyebrow instead of the default', () => {
    render(<PageHeading eyebrow="انبار" title="کالاها" subtitle="فهرست" />);
    expect(screen.getByText('انبار')).toBeTruthy();
    expect(screen.queryByText('مدیریت فروشگاه')).toBeNull();
  });
});

describe('PrimaryButton', () => {
  it('invokes its callback exactly once when clicked', () => {
    const onClick = vi.fn();
    render(<PrimaryButton onClick={onClick}>ثبت کالا</PrimaryButton>);
    fireEvent.click(screen.getByRole('button', { name: 'ثبت کالا' }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
