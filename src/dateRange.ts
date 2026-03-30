const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/;
const MAX_DATE_RANGE_DAYS = 10;

function parseDate(input: string): Date {
  if (!DATE_PATTERN.test(input)) {
    throw new Error(`无效日期格式: ${input}`);
  }

  const [year, month, day] = input.split("-").map((item) => Number(item));
  const parsed = new Date(Date.UTC(year, month - 1, day));

  if (
    parsed.getUTCFullYear() !== year ||
    parsed.getUTCMonth() !== month - 1 ||
    parsed.getUTCDate() !== day
  ) {
    throw new Error(`无效日期值: ${input}`);
  }

  return parsed;
}

function formatDate(date: Date): string {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function expandDateRange(start: string, end: string): string[] {
  const startDate = parseDate(start);
  const endDate = parseDate(end);

  if (startDate > endDate) {
    throw new Error("开始日期不能晚于结束日期");
  }

  const dates: string[] = [];
  const cursor = new Date(startDate);

  while (cursor <= endDate) {
    dates.push(formatDate(cursor));
    cursor.setUTCDate(cursor.getUTCDate() + 1);
  }

  if (dates.length > MAX_DATE_RANGE_DAYS) {
    throw new Error(`日期区间不能超过 ${MAX_DATE_RANGE_DAYS} 天`);
  }

  return dates;
}
