import React, { useState, useEffect } from "react";
import { ApiClient } from "../utils/apiClient";

const apiClient = new ApiClient();

interface SettlementStats {
  total_items_queued: number;
  total_batches_processed: number;
  items_in_current_batch: number;
  last_batch_processed_at: string | null;
  queue_status: string;
}

interface QueueActivity {
  timestamp: Date;
  type: "batch_processed" | "items_queued";
  count: number;
  processingTime?: number;
}

export const SettlementQueueMonitor: React.FC = () => {
  const [stats, setStats] = useState<SettlementStats | null>(null);
  const [activity, setActivity] = useState<QueueActivity[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let intervalId: NodeJS.Timeout;

    const fetchStats = async () => {
      try {
        const { data: newStats } = await apiClient.get("/v1/settlement-stats");

        // Track activity changes
        if (stats) {
          if (
            newStats.total_batches_processed > stats.total_batches_processed
          ) {
            const newActivity: QueueActivity = {
              timestamp: new Date(),
              type: "batch_processed",
              count:
                newStats.total_batches_processed -
                stats.total_batches_processed,
            };
            setActivity((prev) => [newActivity, ...prev.slice(0, 9)]); // Keep last 10 activities
          }

          if (newStats.total_items_queued > stats.total_items_queued) {
            const newActivity: QueueActivity = {
              timestamp: new Date(),
              type: "items_queued",
              count: newStats.total_items_queued - stats.total_items_queued,
            };
            setActivity((prev) => [newActivity, ...prev.slice(0, 9)]);
          }
        }

        setStats(newStats);
        setIsConnected(true);
        setError(null);
      } catch (err) {
        setError("Failed to fetch settlement stats");
        setIsConnected(false);
        console.error("Settlement stats fetch error:", err);
      }
    };

    // Initial fetch
    fetchStats();

    // Poll every 200ms for real-time updates
    intervalId = setInterval(fetchStats, 200);

    return () => {
      if (intervalId) clearInterval(intervalId);
    };
  }, [stats]);

  const formatTime = (timestamp: string | null) => {
    if (!timestamp) return "Never";
    return new Date(timestamp).toLocaleTimeString();
  };

  const formatRelativeTime = (date: Date) => {
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSec = Math.floor(diffMs / 1000);

    if (diffSec < 60) return `${diffSec}s ago`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHour = Math.floor(diffMin / 60);
    return `${diffHour}h ago`;
  };

  const handleRefresh = async () => {
    try {
      const { data: newStats } = await apiClient.get("/v1/settlement-stats");
      setStats(newStats);
      setIsConnected(true);
      setError(null);
    } catch (err) {
      setError("Failed to fetch settlement stats");
      setIsConnected(false);
      console.error("Manual refresh error:", err);
    }
  };

  if (error) {
    return (
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <h2 className="text-xl font-semibold text-gray-900 mb-4">
          Settlement Queue Monitor
        </h2>
        <div className="text-red-600 bg-red-50 p-4 rounded-lg">
          <p>❌ {error}</p>
          <p className="text-sm mt-2">
            Make sure the sequencer is running on localhost:3000
          </p>
        </div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <h2 className="text-xl font-semibold text-gray-900 mb-4">
          Settlement Queue Monitor
        </h2>
        <div className="animate-pulse">
          <div className="h-4 bg-gray-200 rounded w-3/4 mb-2"></div>
          <div className="h-4 bg-gray-200 rounded w-1/2"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-lg shadow-sm border border-gray-200 p-4">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-lg font-semibold text-gray-900">
          Settlement Queue
        </h2>
        <div className="flex items-center space-x-3">
          <button
            onClick={handleRefresh}
            className="px-2 py-1 text-xs bg-blue-50 text-blue-600 rounded hover:bg-blue-100 transition-colors flex items-center space-x-1"
            title="Refresh settlement stats"
          >
            <svg
              className="w-3 h-3"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            <span>Refresh</span>
          </button>
          <div className="flex items-center space-x-2">
            <div
              className={`w-2 h-2 rounded-full ${
                isConnected ? "bg-green-500" : "bg-red-500"
              }`}
            ></div>
            <span className="text-xs text-gray-600">
              {isConnected ? "Live" : "Offline"}
            </span>
          </div>
        </div>
      </div>

      {/* Compact Stats Grid */}
      <div className="grid grid-cols-2 gap-3 mb-4">
        <div className="bg-blue-50 p-3 rounded-lg">
          <div className="text-xl font-bold text-blue-600">
            {stats.total_items_queued}
          </div>
          <div className="text-xs text-blue-600">Items Queued</div>
        </div>

        <div className="bg-green-50 p-3 rounded-lg">
          <div className="text-xl font-bold text-green-600">
            {stats.total_batches_processed}
          </div>
          <div className="text-xs text-green-600">Batches Processed</div>
        </div>

        <div className="bg-purple-50 p-3 rounded-lg">
          <div className="text-xl font-bold text-purple-600">
            {stats.items_in_current_batch}
          </div>
          <div className="text-xs text-purple-600">Current Batch</div>
        </div>

        <div className="bg-amber-50 p-3 rounded-lg">
          <div className="text-lg font-bold text-amber-600">
            {stats.queue_status === "active" ? "🟢" : "🔴"}
          </div>
          <div className="text-xs text-amber-600 capitalize">
            {stats.queue_status}
          </div>
        </div>
      </div>

      {/* Compact Last Batch Info */}
      <div className="bg-gray-50 p-3 rounded-lg mb-4">
        <div className="text-xs font-medium text-gray-700 mb-1">Last Batch</div>
        <div className="text-sm font-semibold text-gray-900">
          {formatTime(stats.last_batch_processed_at)}
        </div>
      </div>

      {/* Compact Activity Feed */}
      <div>
        <h3 className="text-sm font-medium text-gray-900 mb-2">
          Recent Activity
        </h3>
        {activity.length === 0 ? (
          <div className="text-gray-500 text-xs py-2 text-center">
            No recent activity
          </div>
        ) : (
          <div className="space-y-1 max-h-32 overflow-y-auto">
            {activity.slice(0, 5).map((item, index) => (
              <div
                key={index}
                className={`flex items-center justify-between p-2 rounded text-xs ${
                  item.type === "batch_processed"
                    ? "bg-green-50 border border-green-200"
                    : "bg-blue-50 border border-blue-200"
                }`}
              >
                <div className="flex items-center space-x-2">
                  <div
                    className={`text-sm ${
                      item.type === "batch_processed"
                        ? "text-green-600"
                        : "text-blue-600"
                    }`}
                  >
                    {item.type === "batch_processed" ? "⚡" : "📥"}
                  </div>
                  <div className="font-medium text-gray-900">
                    {item.type === "batch_processed" ? "Batch" : "Queued"} (
                    {item.count})
                  </div>
                </div>
                <div className="text-gray-500">
                  {formatRelativeTime(item.timestamp)}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
