export function difference<T>(arr1: T[], arr2: T[]): T[] {
  return arr1.filter((x) => !arr2.includes(x));
}

export function intersection<T>(arr1: T[], arr2: T[]): T[] {
  return arr1.filter((x) => arr2.includes(x));
}
