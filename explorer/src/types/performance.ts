// Performance testing types for ZK Casino dashboard

export interface TestMetrics {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  averageLatency: number;
  minLatency: number;
  maxLatency: number;
  p50Latency: number;
  p95Latency: number;
  p99Latency: number;
  rps: number;
  startTime: Date;
  endTime?: Date;
  duration: number;
  successRate: number;
  errorRate: number;
  // Bet outcome statistics
  headsResults: number;
  tailsResults: number;
  headsGuesses: number;
  tailsGuesses: number;
  correctGuesses: number;
  totalPayout: number;
}

export interface TestConfig {
  testType: 'burst' | 'sustained' | 'custom';
  totalRequests?: number;
  rps?: number;
  duration?: number;
  concurrent?: boolean;
  endpoint?: string;
  payload?: any;
}

export interface TestResult {
  id: string;
  config: TestConfig;
  metrics: TestMetrics;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  errors: string[];
  createdAt: Date;
  completedAt?: Date;
}

export interface RequestResult {
  requestId: number;
  startTime: number;
  endTime: number;
  latency: number;
  success: boolean;
  error?: string;
  statusCode?: number;
  // Bet-specific data
  betData?: {
    bet_id: string;
    guess: boolean; // true = heads, false = tails
    result: boolean; // true = heads, false = tails
    won: boolean;
    payout: number;
  };
}

export interface LiveMetrics {
  currentRps: number;
  currentLatency: number;
  requestsCompleted: number;
  requestsFailed: number;
  elapsedTime: number;
  estimatedTimeRemaining: number;
  headsResults: number;
  tailsResults: number;
  correctGuesses: number;
  totalPayout: number;
}

export interface ChartDataPoint {
  timestamp: number;
  rps: number;
  latency: number;
  errors: number;
}