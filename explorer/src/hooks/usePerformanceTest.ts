// Performance testing hook for ZK Casino dashboard

import { useState, useCallback, useRef } from "react";
import {
  TestConfig,
  TestResult,
  TestMetrics,
  RequestResult,
  LiveMetrics,
  ChartDataPoint,
} from "../types/performance";
import { ApiClient } from "../utils/apiClient";

export const usePerformanceTest = () => {
  const [currentTest, setCurrentTest] = useState<TestResult | null>(null);
  const [testHistory, setTestHistory] = useState<TestResult[]>([]);
  const [liveMetrics, setLiveMetrics] = useState<LiveMetrics | null>(null);
  const [chartData, setChartData] = useState<ChartDataPoint[]>([]);
  const [isRunning, setIsRunning] = useState(false);

  const apiClient = useRef(new ApiClient());
  const abortController = useRef<AbortController | null>(null);
  const metricsInterval = useRef<NodeJS.Timeout | null>(null);

  const calculateMetrics = useCallback(
    (results: RequestResult[]): TestMetrics => {
      const successful = results.filter((r) => r.success);
      const failed = results.filter((r) => !r.success);
      const latencies = successful.map((r) => r.latency).sort((a, b) => a - b);

      const startTime = Math.min(...results.map((r) => r.startTime));
      const endTime = Math.max(...results.map((r) => r.endTime));
      const duration = (endTime - startTime) / 1000; // Convert to seconds

      // Calculate bet outcome statistics
      const betsWithData = successful.filter((r) => r.betData);
      const headsResults = betsWithData.filter(
        (r) => r.betData!.result === true
      ).length;
      const tailsResults = betsWithData.filter(
        (r) => r.betData!.result === false
      ).length;
      const headsGuesses = betsWithData.filter(
        (r) => r.betData!.guess === true
      ).length;
      const tailsGuesses = betsWithData.filter(
        (r) => r.betData!.guess === false
      ).length;
      const correctGuesses = betsWithData.filter((r) => r.betData!.won).length;
      const totalPayout = betsWithData.reduce(
        (sum, r) => sum + (r.betData!.payout || 0),
        0
      );

      return {
        totalRequests: results.length,
        successfulRequests: successful.length,
        failedRequests: failed.length,
        averageLatency:
          latencies.length > 0
            ? latencies.reduce((a, b) => a + b, 0) / latencies.length
            : 0,
        minLatency: latencies.length > 0 ? Math.min(...latencies) : 0,
        maxLatency: latencies.length > 0 ? Math.max(...latencies) : 0,
        p50Latency:
          latencies.length > 0
            ? latencies[Math.floor(latencies.length * 0.5)]
            : 0,
        p95Latency:
          latencies.length > 0
            ? latencies[Math.floor(latencies.length * 0.95)]
            : 0,
        p99Latency:
          latencies.length > 0
            ? latencies[Math.floor(latencies.length * 0.99)]
            : 0,
        rps: duration > 0 ? successful.length / duration : 0,
        startTime: new Date(startTime),
        endTime: new Date(endTime),
        duration,
        successRate:
          results.length > 0 ? (successful.length / results.length) * 100 : 0,
        errorRate:
          results.length > 0 ? (failed.length / results.length) * 100 : 0,
        headsResults,
        tailsResults,
        headsGuesses,
        tailsGuesses,
        correctGuesses,
        totalPayout,
      };
    },
    []
  );

  const updateLiveMetrics = useCallback(
    (results: RequestResult[], startTime: number) => {
      const now = performance.now();
      const elapsed = (now - startTime) / 1000;
      const successful = results.filter((r) => r.success);
      const failed = results.filter((r) => !r.success);

      const currentRps = elapsed > 0 ? successful.length / elapsed : 0;
      const recentResults = results.slice(-10); // Last 10 requests for current latency
      const currentLatency =
        recentResults.length > 0
          ? recentResults.reduce((sum, r) => sum + r.latency, 0) /
            recentResults.length
          : 0;

      // Calculate bet outcome metrics from completed results
      const betMetrics = results.reduce(
        (acc, result) => {
          if (result.betData) {
            if (result.betData.result) acc.headsResults++;
            else acc.tailsResults++;
            if (result.betData.guess === result.betData.result)
              acc.correctGuesses++;
            acc.totalPayout += result.betData.payout || 0;
          }
          return acc;
        },
        { headsResults: 0, tailsResults: 0, correctGuesses: 0, totalPayout: 0 }
      );

      setLiveMetrics({
        currentRps: currentRps,
        currentLatency: currentLatency,
        requestsCompleted: successful.length,
        requestsFailed: failed.length,
        elapsedTime: (Date.now() - startTime) / 1000,
        estimatedTimeRemaining: 0,
        ...betMetrics,
      });

      // Update chart data
      setChartData((prev) => {
        const newPoint: ChartDataPoint = {
          timestamp: now,
          rps: currentRps,
          latency: currentLatency,
          errors: failed.length,
        };
        return [...prev.slice(-50), newPoint]; // Keep last 50 points
      });
    },
    []
  );

  const runBurstTest = useCallback(
    async (requestCount: number) => {
      const testId = `burst_${Date.now()}`;
      const config: TestConfig = {
        testType: "burst",
        totalRequests: requestCount,
        concurrent: true,
      };

      const testResult: TestResult = {
        id: testId,
        config,
        metrics: {} as TestMetrics,
        status: "running",
        errors: [],
        createdAt: new Date(),
      };

      setCurrentTest(testResult);
      setIsRunning(true);
      setLiveMetrics(null);
      setChartData([]);

      try {
        abortController.current = new AbortController();
        const startTime = performance.now();
        const results: RequestResult[] = [];

        // Create all requests concurrently
        const promises = Array.from(
          { length: requestCount },
          async (_, index) => {
            const requestStartTime = performance.now();
            try {
              const bet = apiClient.current.generateRandomBet();
              const { response, latency } = await apiClient.current.placeBet(
                bet
              );
              const requestEndTime = performance.now();

              const result: RequestResult = {
                requestId: index,
                startTime: requestStartTime,
                endTime: requestEndTime,
                latency,
                success: true,
                betData: {
                  bet_id: response.bet_id,
                  guess: response.guess,
                  result: response.result,
                  won: response.won,
                  payout: response.payout,
                },
              };

              results.push(result);
              updateLiveMetrics(results, startTime);
              return result;
            } catch (error) {
              const requestEndTime = performance.now();
              const result: RequestResult = {
                requestId: index,
                startTime: requestStartTime,
                endTime: requestEndTime,
                latency: requestEndTime - requestStartTime,
                success: false,
                error: error instanceof Error ? error.message : "Unknown error",
              };

              results.push(result);
              updateLiveMetrics(results, startTime);
              return result;
            }
          }
        );

        await Promise.all(promises);

        const metrics = calculateMetrics(results);
        const completedTest: TestResult = {
          ...testResult,
          metrics,
          status: "completed",
          completedAt: new Date(),
        };

        setCurrentTest(completedTest);
        setTestHistory((prev) => [completedTest, ...prev]);
      } catch (error) {
        const failedTest: TestResult = {
          ...testResult,
          status: "failed",
          errors: [error instanceof Error ? error.message : "Test failed"],
          completedAt: new Date(),
        };

        setCurrentTest(failedTest);
        setTestHistory((prev) => [failedTest, ...prev]);
      } finally {
        setIsRunning(false);
        if (metricsInterval.current) {
          clearInterval(metricsInterval.current);
        }
      }
    },
    [calculateMetrics, updateLiveMetrics]
  );

  const runSustainedTest = useCallback(
    async (targetRps: number, durationSeconds: number) => {
      const testId = `sustained_${Date.now()}`;
      const config: TestConfig = {
        testType: "sustained",
        rps: targetRps,
        duration: durationSeconds,
        concurrent: false,
      };

      const testResult: TestResult = {
        id: testId,
        config,
        metrics: {} as TestMetrics,
        status: "running",
        errors: [],
        createdAt: new Date(),
      };

      setCurrentTest(testResult);
      setIsRunning(true);
      setLiveMetrics(null);
      setChartData([]);

      try {
        abortController.current = new AbortController();
        const startTime = performance.now();
        const results: RequestResult[] = [];
        const intervalMs = 1000 / targetRps; // Time between requests

        let requestCount = 0;
        const maxRequests = targetRps * durationSeconds;

        const sendRequest = async () => {
          if (
            requestCount >= maxRequests ||
            abortController.current?.signal.aborted
          ) {
            return;
          }

          const requestStartTime = performance.now();
          const requestId = requestCount++;

          try {
            const bet = apiClient.current.generateRandomBet();
            const { response, latency } = await apiClient.current.placeBet(bet);
            const requestEndTime = performance.now();

            const result: RequestResult = {
              requestId,
              startTime: requestStartTime,
              endTime: requestEndTime,
              latency,
              success: true,
              betData: {
                bet_id: response.bet_id,
                guess: response.guess,
                result: response.result,
                won: response.won,
                payout: response.payout,
              },
            };

            results.push(result);
            updateLiveMetrics(results, startTime);
          } catch (error) {
            const requestEndTime = performance.now();
            const result: RequestResult = {
              requestId,
              startTime: requestStartTime,
              endTime: requestEndTime,
              latency: requestEndTime - requestStartTime,
              success: false,
              error: error instanceof Error ? error.message : "Unknown error",
            };

            results.push(result);
            updateLiveMetrics(results, startTime);
          }

          // Schedule next request
          if (
            requestCount < maxRequests &&
            !abortController.current?.signal.aborted
          ) {
            setTimeout(sendRequest, intervalMs);
          }
        };

        // Start the sustained test
        sendRequest();

        // Wait for test completion
        return new Promise<void>((resolve) => {
          const checkCompletion = () => {
            if (
              requestCount >= maxRequests ||
              abortController.current?.signal.aborted
            ) {
              const metrics = calculateMetrics(results);
              const completedTest: TestResult = {
                ...testResult,
                metrics,
                status: "completed",
                completedAt: new Date(),
              };

              setCurrentTest(completedTest);
              setTestHistory((prev) => [completedTest, ...prev]);
              resolve();
            } else {
              setTimeout(checkCompletion, 100);
            }
          };

          setTimeout(checkCompletion, durationSeconds * 1000 + 1000);
        });
      } catch (error) {
        const failedTest: TestResult = {
          ...testResult,
          status: "failed",
          errors: [error instanceof Error ? error.message : "Test failed"],
          completedAt: new Date(),
        };

        setCurrentTest(failedTest);
        setTestHistory((prev) => [failedTest, ...prev]);
      } finally {
        setIsRunning(false);
      }
    },
    [calculateMetrics, updateLiveMetrics]
  );

  const cancelTest = useCallback(() => {
    if (abortController.current) {
      abortController.current.abort();
    }
    setIsRunning(false);
    if (currentTest && currentTest.status === "running") {
      const cancelledTest: TestResult = {
        ...currentTest,
        status: "cancelled",
        completedAt: new Date(),
      };
      setCurrentTest(cancelledTest);
      setTestHistory((prev) => [cancelledTest, ...prev]);
    }
  }, [currentTest]);

  const clearHistory = useCallback(() => {
    setTestHistory([]);
  }, []);

  return {
    currentTest,
    testHistory,
    liveMetrics,
    chartData,
    isRunning,
    runBurstTest,
    runSustainedTest,
    cancelTest,
    clearHistory,
  };
};
