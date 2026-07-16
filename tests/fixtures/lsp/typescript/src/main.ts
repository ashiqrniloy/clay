export function answer(): number {
  return 42;
}

export function greet(name: string): string {
  return `hello ${name}`;
}

export function main(): number {
  const result = answer();
  return result;
}
