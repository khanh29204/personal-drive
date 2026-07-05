export const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exponent;
  return `${value.toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
};

/**
 * Chọn class icon FontAwesome (fa-solid) theo mimeType, dùng để hiển thị
 * trong danh sách file trên view EJS.
 */
export const getFileIcon = (mimeType: string): string => {
  if (mimeType.startsWith('image/')) return 'fa-file-image';
  if (mimeType.startsWith('video/')) return 'fa-file-video';
  if (mimeType.startsWith('audio/')) return 'fa-file-audio';
  if (mimeType === 'application/pdf') return 'fa-file-pdf';
  if (mimeType.includes('zip') || mimeType.includes('rar') || mimeType.includes('compressed')) {
    return 'fa-file-archive';
  }
  if (mimeType.includes('word')) return 'fa-file-word';
  if (mimeType.includes('excel') || mimeType.includes('spreadsheet')) return 'fa-file-excel';
  if (mimeType.startsWith('text/')) return 'fa-file-alt';
  return 'fa-file';
};
