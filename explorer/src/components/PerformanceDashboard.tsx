// Performance testing dashboard for ZK Casino

import React, { useState } from 'react';
import { usePerformanceTest } from '../hooks/usePerformanceTest';
import { PerformanceChart } from './PerformanceChart';
import { TestResults } from './TestResults';

export const PerformanceDashboard: React.FC = () => {
  const {
    currentTest,
    testHistory,
    liveMetrics,
    chartData,
    isRunning,
    runBurstTest,
    runSustainedTest,
    cancelTest,
    clearHistory,
  } = usePerformanceTest();

  const [selectedTestType, setSelectedTestType] = useState<'burst' | 'sustained'>('burst');
  const [burstRequests, setBurstRequests] = useState(100);
  const [sustainedRps, setSustainedRps] = useState(50);
  const [sustainedDuration, setSustainedDuration] = useState(10);

  const handleRunTest = async () => {
    if (selectedTestType === 'burst') {
      await runBurstTest(burstRequests);
    } else {
      await runSustainedTest(sustainedRps, sustainedDuration);
    }
  };

  const formatMetric = (value: number, decimals = 2) => {
    return value.toFixed(decimals);
  };

  return (
    <div className="performance-dashboard max-w-7xl mx-auto p-6 space-y-6">
      {/* Header */}
      <div className="text-center">
        <h1 className="text-3xl font-bold text-gray-900 mb-2">
          ZK Casino Performance Dashboard
        </h1>
        <p className="text-gray-600">
          Test and monitor the high-performance sequencer with real-time metrics
        </p>
      </div>

      {/* Test Configuration */}
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <h2 className="text-xl font-semibold text-gray-900 mb-4">Test Configuration</h2>
        
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Test Type Selection */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Test Type
            </label>
            <select
              value={selectedTestType}
              onChange={(e) => setSelectedTestType(e.target.value as 'burst' | 'sustained')}
              disabled={isRunning}
              className="w-full p-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            >
              <option value="burst">Burst Test (Concurrent Requests)</option>
              <option value="sustained">Sustained Test (Continuous RPS)</option>
            </select>
          </div>

          {/* Test Parameters */}
          <div>
            {selectedTestType === 'burst' ? (
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">
                  Total Requests
                </label>
                <select
                  value={burstRequests}
                  onChange={(e) => setBurstRequests(Number(e.target.value))}
                  disabled={isRunning}
                  className="w-full p-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                >
                  <option value={10}>10 requests</option>
                  <option value={50}>50 requests</option>
                  <option value={100}>100 requests</option>
                  <option value={250}>250 requests</option>
                  <option value={500}>500 requests</option>
                  <option value={1000}>1000 requests</option>
                  <option value={2000}>2000 requests</option>
                </select>
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Target RPS
                  </label>
                  <select
                    value={sustainedRps}
                    onChange={(e) => setSustainedRps(Number(e.target.value))}
                    disabled={isRunning}
                    className="w-full p-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  >
                    <option value={10}>10 RPS</option>
                    <option value={25}>25 RPS</option>
                    <option value={50}>50 RPS</option>
                    <option value={100}>100 RPS</option>
                    <option value={200}>200 RPS</option>
                    <option value={500}>500 RPS</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Duration (seconds)
                  </label>
                  <select
                    value={sustainedDuration}
                    onChange={(e) => setSustainedDuration(Number(e.target.value))}
                    disabled={isRunning}
                    className="w-full p-2 border border-gray-300 rounded-md focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                  >
                    <option value={5}>5 seconds</option>
                    <option value={10}>10 seconds</option>
                    <option value={30}>30 seconds</option>
                    <option value={60}>60 seconds</option>
                    <option value={120}>2 minutes</option>
                    <option value={300}>5 minutes</option>
                  </select>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex gap-3 mt-6">
          <button
            onClick={handleRunTest}
            disabled={isRunning}
            className={`px-6 py-2 rounded-md font-medium transition-colors ${
              isRunning
                ? 'bg-gray-300 text-gray-500 cursor-not-allowed'
                : 'bg-blue-600 text-white hover:bg-blue-700'
            }`}
          >
            {isRunning ? 'Running Test...' : 'Start Test'}
          </button>
          
          {isRunning && (
            <button
              onClick={cancelTest}
              className="px-6 py-2 bg-red-600 text-white rounded-md font-medium hover:bg-red-700 transition-colors"
            >
              Cancel Test
            </button>
          )}
          
          {testHistory.length > 0 && !isRunning && (
            <button
              onClick={clearHistory}
              className="px-6 py-2 bg-gray-600 text-white rounded-md font-medium hover:bg-gray-700 transition-colors"
            >
              Clear History
            </button>
          )}
        </div>
      </div>

      {/* Live Metrics */}
      {liveMetrics && (
        <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
          <h2 className="text-xl font-semibold text-gray-900 mb-4">Live Metrics</h2>
          
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <div className="text-2xl font-bold text-blue-600">
                {formatMetric(liveMetrics.currentRps)}
              </div>
              <div className="text-sm text-blue-600">Current RPS</div>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <div className="text-2xl font-bold text-green-600">
                {formatMetric(liveMetrics.currentLatency)}ms
              </div>
              <div className="text-sm text-green-600">Current Latency</div>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <div className="text-2xl font-bold text-purple-600">
                {liveMetrics.requestsCompleted}
              </div>
              <div className="text-sm text-purple-600">Completed</div>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <div className="text-2xl font-bold text-red-600">
                {liveMetrics.requestsFailed}
              </div>
              <div className="text-sm text-red-600">Failed</div>
            </div>
          </div>
          
          <div className="mt-4 text-sm text-gray-600">
            Elapsed Time: {formatMetric(liveMetrics.elapsedTime)}s
          </div>
        </div>
      )}

      {/* Performance Chart */}
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <h2 className="text-xl font-semibold text-gray-900 mb-4">Real-time Performance</h2>
        <PerformanceChart data={chartData} height={400} />
      </div>

      {/* Current Test Results */}
      {currentTest && (
        <div>
          <h2 className="text-xl font-semibold text-gray-900 mb-4">Current Test</h2>
          <TestResults testResult={currentTest} />
        </div>
      )}

      {/* Test History */}
      {testHistory.length > 0 && (
        <div>
          <h2 className="text-xl font-semibold text-gray-900 mb-4">
            Test History ({testHistory.length})
          </h2>
          <div className="space-y-4">
            {testHistory.slice(0, 5).map((test) => (
              <TestResults key={test.id} testResult={test} />
            ))}
            {testHistory.length > 5 && (
              <div className="text-center py-4 text-gray-500">
                ... and {testHistory.length - 5} more tests
              </div>
            )}
          </div>
        </div>
      )}

      {/* Performance Targets */}
      <div className="bg-gradient-to-r from-blue-50 to-purple-50 rounded-lg border border-blue-200 p-6">
        <h2 className="text-xl font-semibold text-gray-900 mb-4">Performance Targets</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          <div className="text-center">
            <div className="text-3xl font-bold text-blue-600">656+ RPS</div>
            <div className="text-sm text-gray-600 mt-1">Current Achievement</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-green-600">&lt;300ms</div>
            <div className="text-sm text-gray-600 mt-1">P95 Latency Target</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-purple-600">3000+ RPS</div>
            <div className="text-sm text-gray-600 mt-1">VF Node Target</div>
          </div>
        </div>
      </div>
    </div>
  );
};