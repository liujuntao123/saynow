export type RecognitionStatus = 'success' | 'failed' | 'processing';

export interface RecognitionRecordInput {
  durationSeconds: number;
  text: string;
  status: RecognitionStatus;
  createdAt: string;
}

export interface UsageStats {
  totalDurationSeconds: number;
  totalRecords: number;
  totalCharacters: number;
}

export function aggregateUsageStats(records: RecognitionRecordInput[]): UsageStats {
  return records
    .filter((record) => record.status === 'success')
    .reduce<UsageStats>(
      (stats, record) => ({
        totalDurationSeconds: stats.totalDurationSeconds + Math.max(0, record.durationSeconds),
        totalRecords: stats.totalRecords + 1,
        totalCharacters: stats.totalCharacters + Array.from(record.text).length,
      }),
      { totalDurationSeconds: 0, totalRecords: 0, totalCharacters: 0 },
    );
}
