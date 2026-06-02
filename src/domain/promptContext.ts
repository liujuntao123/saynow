interface VocabularyItem {
  term: string;
  alias?: string;
  enabled: boolean;
}

interface StylePrompt {
  name: string;
  prompt: string;
  enabled: boolean;
}

interface HistoryRecord {
  text: string;
  status: 'success' | 'failed' | 'processing';
}

export interface PromptContextInput {
  vocabulary: VocabularyItem[];
  styles: StylePrompt[];
  records: HistoryRecord[];
}

export function buildPromptPreview(input: PromptContextInput): string {
  const vocabulary = input.vocabulary
    .filter((item) => item.enabled)
    .slice(0, 30)
    .map((item) => `- ${item.term}${item.alias ? ` (${item.alias})` : ''}`)
    .join('\n');

  const style = input.styles.find((item) => item.enabled)?.prompt ?? '输出自然、准确的简体中文文本。';

  const history = input.records
    .filter((record) => record.status === 'success' && record.text.trim())
    .slice(0, 5)
    .map((record) => `- ${record.text.trim()}`)
    .join('\n');

  return [
    '你是一个桌面端语音识别助手。只输出最终识别文本，不输出解释。',
    `输出风格：${style}`,
    vocabulary ? `用户词库：\n${vocabulary}` : '',
    history ? `相关历史：\n${history}` : '',
  ]
    .filter(Boolean)
    .join('\n\n');
}
