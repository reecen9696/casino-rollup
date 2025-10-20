// API client for ZK Casino performance testing

export interface BetRequest {
  player_address: string;
  amount: number;
  guess: boolean;
}

export interface BetResponse {
  bet_id: string;
  player_address: string;
  amount: number;
  guess: boolean;
  result: boolean;
  won: boolean;
  payout: number;
  timestamp: string;
}

export class ApiClient {
  private baseUrl: string;

  constructor(baseUrl: string = "http://localhost:3000") {
    this.baseUrl = baseUrl;
  }

  async get(endpoint: string): Promise<{ data: any; latency: number }> {
    const startTime = performance.now();

    try {
      const response = await fetch(`${this.baseUrl}${endpoint}`);
      const endTime = performance.now();

      if (!response.ok) {
        throw new Error(`GET ${endpoint} failed: ${response.status}`);
      }

      const data = await response.json();
      return {
        data,
        latency: endTime - startTime,
      };
    } catch (error) {
      throw new Error(
        `GET error: ${error instanceof Error ? error.message : "Unknown error"}`
      );
    }
  }

  async healthCheck(): Promise<{ status: string; latency: number }> {
    const startTime = performance.now();

    try {
      const response = await fetch(`${this.baseUrl}/health`);
      const endTime = performance.now();

      if (!response.ok) {
        throw new Error(`Health check failed: ${response.status}`);
      }

      const text = await response.text();
      return {
        status: text,
        latency: endTime - startTime,
      };
    } catch (error) {
      throw new Error(
        `Health check error: ${
          error instanceof Error ? error.message : "Unknown error"
        }`
      );
    }
  }

  async placeBet(
    bet: BetRequest
  ): Promise<{ response: BetResponse; latency: number }> {
    const startTime = performance.now();

    try {
      const response = await fetch(`${this.baseUrl}/v1/bet`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(bet),
      });

      const endTime = performance.now();

      if (!response.ok) {
        throw new Error(
          `Bet failed: ${response.status} ${response.statusText}`
        );
      }

      const data = await response.json();
      return {
        response: data,
        latency: endTime - startTime,
      };
    } catch (error) {
      throw new Error(
        `Bet error: ${error instanceof Error ? error.message : "Unknown error"}`
      );
    }
  }

  async getRecentBets(): Promise<{ bets: any[]; latency: number }> {
    const startTime = performance.now();

    try {
      const response = await fetch(`${this.baseUrl}/v1/bets`);
      const endTime = performance.now();

      if (!response.ok) {
        throw new Error(`Failed to get bets: ${response.status}`);
      }

      const data = await response.json();
      return {
        bets: data.bets || [],
        latency: endTime - startTime,
      };
    } catch (error) {
      throw new Error(
        `Get bets error: ${
          error instanceof Error ? error.message : "Unknown error"
        }`
      );
    }
  }

  // Generate a random bet request for testing
  generateRandomBet(): BetRequest {
    return {
      player_address: `test_player_${Math.random().toString(36).substr(2, 9)}`,
      amount: Math.floor(Math.random() * 50000) + 10000, // 10k-60k lamports
      guess: Math.random() > 0.5,
    };
  }
}
