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

const FORMAT_EXAMPLE =
  '格式示例：将“上周三，也就是六月三号，我上午九点零五分参加了第二次产品评审，讨论了三个方案、十二条反馈和百分之十五的预算调整。下午，我把Meeting Notes发给了Alice，晚上八点半又确认了一遍OKR”输出为“上周三，也就是6月3号，我上午9:05参加了第2次产品评审，讨论了3个方案、12条反馈和15%的预算调整。下午，我把Meeting Notes发给了Alice，晚上8:30又确认了一遍OKR”。';
const CLEANUP_RULES =
  '整理规则：\n- 先准确识别音频，再做轻度语句整理。\n- 保留说话者原本的情绪、语气和表达强度；保留自然语气词，例如“嗯”“啊”“吧”“嘛”“呢”等。\n- 清理明显口误、卡顿、误触发和无意义重复，让句子更通顺、更易读。\n- 不把自然口语强行改成正式书面语；不补充、不删改原意，不添加音频里没有的信息。';
const CLEANUP_EXAMPLE =
  '口语整理示例：将“嗯我觉得这个方案吧，就是就是还挺顺的，然后呃我们明天再看一下。”输出为“嗯，我觉得这个方案吧，还挺顺的，然后我们明天再看一下。”。';

export function buildPromptPreview(input: PromptContextInput): string {
  const vocabulary = input.vocabulary
    .filter((item) => item.enabled)
    .slice(0, 30)
    .map((item) => `- ${item.term}${item.alias ? ` (${item.alias})` : ''}`)
    .join('\n');

  const style =
    input.styles.find((item) => item.enabled)?.prompt ??
    '在保留说话者情绪和自然语气词的基础上，输出准确、通顺、易读的简体中文文本。';

  const history = input.records
    .filter((record) => record.status === 'success' && record.text.trim())
    .slice(0, 5)
    .map((record) => `- ${record.text.trim()}`)
    .join('\n');

  return [
    '你是一个桌面端语音识别助手。只输出最终识别文本，不输出解释。',
    `输出风格：${style}`,
    CLEANUP_RULES,
    FORMAT_EXAMPLE,
    CLEANUP_EXAMPLE,
    vocabulary ? `用户词库：\n${vocabulary}` : '',
    history ? `相关历史：\n${history}` : '',
  ]
    .filter(Boolean)
    .join('\n\n');
}
