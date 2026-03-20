export function createOmniEngine(config?: unknown): Promise<{
    distill: (text: string) => string;
}>;

export function readFixture(name: string): string;
