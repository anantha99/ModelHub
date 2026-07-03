const byteUnits = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number | null, unavailableLabel = "Size unavailable"): string {
  if (bytes === null) {
    return unavailableLabel;
  }

  if (bytes === 0) {
    return joinNumberAndUnit(0, "B");
  }

  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), byteUnits.length - 1);
  const value = bytes / 1024 ** unitIndex;
  const maximumFractionDigits = value >= 10 || unitIndex === 0 ? 0 : 1;

  return joinNumberAndUnit(formatNumber(value, maximumFractionDigits), byteUnits[unitIndex]);
}

export function formatCount(value: number | null, unavailableLabel = "--"): string {
  return value === null ? unavailableLabel : formatNumber(value, 0);
}

export function formatScanTimestamp(value: string | null): string {
  const date = parseTimestamp(value);

  if (!date) {
    return "time unavailable";
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function formatRelativeTime(value: string | null): string {
  const date = parseTimestamp(value);

  if (!date) {
    return "time unavailable";
  }

  const elapsedSeconds = Math.round((date.getTime() - Date.now()) / 1000);
  const absoluteSeconds = Math.abs(elapsedSeconds);

  if (absoluteSeconds < 45) {
    return "just now";
  }

  const divisions: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ["minute", 60],
    ["hour", 60 * 60],
    ["day", 60 * 60 * 24],
    ["week", 60 * 60 * 24 * 7],
    ["month", 60 * 60 * 24 * 30],
    ["year", 60 * 60 * 24 * 365],
  ];
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

  for (const [unit, secondsInUnit] of divisions) {
    if (absoluteSeconds < secondsInUnit * 1.5 || unit === "year") {
      return formatter.format(Math.round(elapsedSeconds / secondsInUnit), unit);
    }
  }

  return formatter.format(Math.round(elapsedSeconds / 31_536_000), "year");
}

function parseTimestamp(value: string | null): Date | null {
  if (!value || value === "0") {
    return null;
  }

  const numericValue = Number(value);
  const date = Number.isFinite(numericValue)
    ? new Date(numericValue * 1000)
    : new Date(value);

  return Number.isNaN(date.getTime()) ? null : date;
}

function formatNumber(value: number, maximumFractionDigits: number): string {
  return new Intl.NumberFormat(undefined, {
    maximumFractionDigits,
  }).format(value);
}

function joinNumberAndUnit(value: number | string, unit: string): string {
  return `${value}\u00a0${unit}`;
}
