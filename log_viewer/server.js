const express = require('express');
const cors = require('cors');
const multer = require('multer');
const fs = require('fs-extra');
const path = require('path');
const LogPreprocessor = require('./scripts/preprocess_logs');
const MongoDBService = require('./scripts/mongodb_service');

const app = express();
const PORT = process.env.PORT || 3000;

// Initialize preprocessor and MongoDB service
const preprocessor = new LogPreprocessor('./cache');
const mongoService = new MongoDBService();

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.static('public'));

// Configure multer for file uploads
const storage = multer.diskStorage({
    destination: (req, file, cb) => {
        cb(null, './uploads');
    },
    filename: (req, file, cb) => {
        const timestamp = Date.now();
        cb(null, `${timestamp}-${file.originalname}`);
    }
});

const upload = multer({ 
    storage,
    fileFilter: (req, file, cb) => {
        // Accept all file types, but we'll validate content later
        cb(null, true);
    },
    limits: {
        fileSize: 100 * 1024 * 1024 // 100MB limit
    }
});

// API Routes

/**
 * Get list of available Ruse runs (cached and uploaded)
 */
app.get('/api/runs', async (req, res) => {
    try {
        const cached = await preprocessor.listCached();
        
        // Transform cached data to show as Ruse runs
        const runs = cached.map(cache => {
            const metadata = cache.metadata || {};
            return {
                id: cache.cacheHash,
                runTime: metadata.runTime || metadata.processedAt || 'Unknown',
                timestamp: metadata.timestamp || new Date(metadata.processedAt).getTime() / 1000,
                sourceFile: metadata.sourceFile || 'Unknown',
                resultFile: metadata.resultFile || 'Unknown',
                taskCount: metadata.resultMetadata?.taskCount || 0,
                passedTasks: metadata.resultMetadata?.passedTasks || 0,
                failedTasks: metadata.resultMetadata?.failedTasks || 0,
                totalTime: metadata.resultMetadata?.totalTime || 0
            };
        }).sort((a, b) => b.timestamp - a.timestamp); // Sort by timestamp, newest first
        
        res.json({ runs });
    } catch (error) {
        console.error('Error listing runs:', error);
        res.status(500).json({ error: 'Failed to list Ruse runs' });
    }
});

/**
 * Get list of available log files (cached and uploaded) - kept for backward compatibility
 */
app.get('/api/logs', async (req, res) => {
    try {
        const cached = await preprocessor.listCached();
        
        // Also check uploads directory
        const uploadsDir = './uploads';
        let uploaded = [];
        
        if (fs.existsSync(uploadsDir)) {
            const files = await fs.readdir(uploadsDir);
            uploaded = files.map(file => ({
                filename: file,
                path: path.join(uploadsDir, file),
                type: 'uploaded'
            }));
        }
        
        res.json({
            cached: cached,
            uploaded: uploaded
        });
    } catch (error) {
        console.error('Error listing logs:', error);
        res.status(500).json({ error: 'Failed to list log files' });
    }
});

/**
 * Upload and process complete Ruse runs (log file and result file required)
 */
app.post('/api/upload', upload.fields([
    { name: 'logFile', maxCount: 1 },
    { name: 'resultFile', maxCount: 1 }
]), async (req, res) => {
    try {
        if (!req.files || !req.files.logFile) {
            return res.status(400).json({ error: 'Log file is required' });
        }
        
        if (!req.files.resultFile) {
            return res.status(400).json({ error: 'Result file is required for a complete Ruse run' });
        }
        
        const logFilePath = req.files.logFile[0].path;
        const resultFilePath = req.files.resultFile[0].path;
        
        // Process as complete Ruse run
        const processed = await preprocessor.processRuseRun(logFilePath, resultFilePath);
        
        res.json({
            message: 'Ruse run uploaded and processed successfully',
            filename: req.files.logFile[0].filename,
            resultFilename: req.files.resultFile[0].filename,
            metadata: processed.metadata
        });
    } catch (error) {
        console.error('Error processing uploaded files:', error);
        res.status(500).json({ error: 'Failed to process uploaded files' });
    }
});

/**
 * Process an existing file
 */
app.post('/api/process', async (req, res) => {
    try {
        const { filePath } = req.body;
        
        if (!filePath) {
            return res.status(400).json({ error: 'File path is required' });
        }
        
        if (!fs.existsSync(filePath)) {
            return res.status(404).json({ error: 'File not found' });
        }
        
        const processed = await preprocessor.processLogFile(filePath);
        
        res.json({
            message: 'File processed successfully',
            metadata: processed.metadata
        });
    } catch (error) {
        console.error('Error processing file:', error);
        res.status(500).json({ error: 'Failed to process file' });
    }
});

/**
 * Get results data for a specific Ruse run
 */
app.get('/api/runs/:runId/results', async (req, res) => {
    try {
        const { runId } = req.params;
        
        // Get metadata for the run
        const metadata = await preprocessor.mongoService.getMetadata(runId);
        
        if (!metadata || !metadata.resultData) {
            return res.status(404).json({ error: 'Results not found for this run' });
        }
        
        res.json({
            metadata: metadata.resultMetadata,
            results: metadata.resultData,
            runInfo: {
                id: runId,
                runTime: metadata.runTime,
                timestamp: metadata.timestamp,
                sourceFile: metadata.sourceFile,
                resultFile: metadata.resultFile
            }
        });
    } catch (error) {
        console.error('Error getting run results:', error);
        res.status(500).json({ error: 'Failed to get run results' });
    }
});

/**
 * Get processed logs with filtering
 */
app.get('/api/logs/:cacheHash', async (req, res) => {
    try {
        const { cacheHash } = req.params;
        const { 
            page = 1, 
            limit = 100, 
            level, 
            target, 
            search, 
            isPanic, 
            task, 
            iteration,
            threadId
        } = req.query;
        
        // Build filters object
        const filters = {};
        if (level) {
            filters.level = level.split(',');
        }
        if (target) {
            filters.target = target.split(',');
        }
        if (search) {
            filters.search = search;
        }
        if (isPanic) {
            filters.isPanic = isPanic;
        }
        if (task) {
            if (iteration) {
                filters.taskFilter = {
                    type: 'task_iteration',
                    taskName: task.split(','),
                    iteration: iteration.split(',')
                };
            } else {
                filters.taskFilter = {
                    type: 'task',
                    taskName: task.split(',')
                };
            }
        }
        if (threadId) {
            filters.threadId = threadId.split(',');
        }
        
        const pageNum = parseInt(page, 10);
        const limitNum = parseInt(limit, 10);
        
        // Get logs with pagination from the preprocessor
        const logsResult = await preprocessor.readLogsFromCache(cacheHash, pageNum, limitNum, filters);
        
        if (!await preprocessor.mongoService.isAvailable()) {
            return res.json({ 
                enabled: false,
                available: false,
                message: 'MongoDB not running'
            });
        }

        // Get metadata for stats (this also works with MongoDB)
        let metadata;
        try {
            // Try to get metadata from MongoDB first if available
            metadata = await preprocessor.mongoService.getMetadata(cacheHash);
        } catch (error) {
            throw new Error('Failed to get metadata from MongoDB');
        }
        
        res.json({
            metadata: metadata,
            logs: logsResult.logs,
            pagination: logsResult.pagination
        });
    } catch (error) {
        console.error('Error fetching logs:', error);
        res.status(500).json({ error: 'Failed to fetch logs' });
    }
});

/**
 * Get log statistics
 */
app.get('/api/stats/:cacheHash', async (req, res) => {
    try {
        const { cacheHash } = req.params;
        
        let metadata;

        if (!await preprocessor.mongoService.isAvailable()) {
            throw new Error('MongoDB is not available');
        }

        try {
            // Try MongoDB first if available
            metadata = await preprocessor.mongoService.getMetadata(cacheHash);
        } catch (error) {
            throw new Error('Failed to get metadata from MongoDB');
        }
        
        res.json({
            metadata: metadata,
            stats: metadata.stats
        });
    } catch (error) {
        console.error('Error fetching stats:', error);
        res.status(500).json({ error: 'Failed to fetch statistics' });
    }
});

/**
 * Delete a cache file
 */
app.delete('/api/cache/:cacheHash', async (req, res) => {
    try {
        const { cacheHash } = req.params;
        
        let deletedSomething = false;
        if (!await preprocessor.mongoService.isAvailable()) {
            throw new Error('MongoDB is not available');
        }
        // Delete from MongoDB if available
        try {
            await preprocessor.mongoService.deleteCacheData(cacheHash);
            deletedSomething = true;
            console.log(`Deleted MongoDB data for ${cacheHash}`);
        } catch (error) {
            console.warn('Failed to delete from MongoDB:', error.message);
        }
        
        if (!deletedSomething) {
            return res.status(404).json({ error: 'Cache data not found' });
        }
        
        res.json({ message: 'Cache data deleted successfully' });
    } catch (error) {
        console.error('Error deleting cache data:', error);
        res.status(500).json({ error: 'Failed to delete cache data' });
    }
});

/**
 * Get MongoDB status and statistics
 */
app.get('/api/mongodb/status', async (req, res) => {
    try {
        const available = await preprocessor.mongoService.isAvailable();
        
        if (!available) {
            return res.json({
                enabled: true,
                available: false,
                message: 'MongoDB not available. Make sure MongoDB is running on localhost:27017'
            });
        }
        
        const stats = await preprocessor.mongoService.getStats();
        
        // Try to get database-level statistics
        let dbStats = {};
        try {
            dbStats = await preprocessor.mongoService.db.stats();
        } catch (dbStatsError) {
            // Fallback: estimate database stats
            try {
                const collections = await preprocessor.mongoService.db.listCollections().toArray();
                let totalSize = 0;
                let totalObjects = 0;
                
                for (const collection of collections) {
                    try {
                        const coll = preprocessor.mongoService.db.collection(collection.name);
                        const count = await coll.countDocuments();
                        totalObjects += count;
                        
                        // Estimate size
                        const sampleDocs = await coll.find({}).limit(10).toArray();
                        if (sampleDocs.length > 0) {
                            const avgSize = JSON.stringify(sampleDocs).length / sampleDocs.length;
                            totalSize += avgSize * count;
                        }
                    } catch (collError) {
                        console.warn(`Could not get stats for collection ${collection.name}:`, collError.message);
                    }
                }
                
                dbStats = {
                    storageSize: totalSize,
                    dataSize: totalSize * 0.8, // Estimate
                    indexSize: totalSize * 0.2, // Estimate
                    totalSize: totalSize,
                    objects: totalObjects,
                    avgObjSize: totalObjects > 0 ? totalSize / totalObjects : 0
                };
            } catch (fallbackError) {
                console.warn('Could not get database stats:', fallbackError.message);
                dbStats = {
                    storageSize: 0,
                    dataSize: 0,
                    indexSize: 0,
                    totalSize: 0,
                    objects: 0,
                    avgObjSize: 0
                };
            }
        }
        
        res.json({
            enabled: true,
            available: true,
            stats: {
                ...stats,
                storageSize: dbStats.storageSize || 0,
                dataSize: dbStats.dataSize || 0,
                indexSize: dbStats.indexSize || 0,
                totalSize: (dbStats.storageSize || 0) + (dbStats.indexSize || 0),
                objects: dbStats.objects || 0,
                avgObjSize: dbStats.avgObjSize || 0
            },
            connection: 'mongodb://localhost:27017',
            database: 'log_viewer'
        });
    } catch (error) {
        console.error('Error checking MongoDB status:', error);
        res.json({
            enabled: true,
            available: false,
            error: error.message
        });
    }
});

/**
 * Get list of all MongoDB collections with detailed stats
 */
app.get('/api/mongodb/collections', async (req, res) => {
    try {
        const isAvailable = await preprocessor.mongoService.isAvailable();
        if (!isAvailable) {
            return res.status(503).json({ error: 'MongoDB is not available' });
        }
        
        const collections = await preprocessor.mongoService.db.listCollections().toArray();
        const collectionStats = [];
        
        for (const collection of collections) {
            try {
                const coll = preprocessor.mongoService.db.collection(collection.name);
                
                // Get basic count with timeout
                const countPromise = coll.countDocuments();
                const timeoutPromise = new Promise((_, reject) => 
                    setTimeout(() => reject(new Error('Timeout')), 5000)
                );
                
                let count;
                try {
                    count = await Promise.race([countPromise, timeoutPromise]);
                } catch (timeoutError) {
                    console.warn(`Timeout getting count for ${collection.name}`);
                    count = 0;
                }
                
                // Get simple stats without complex operations
                let stats = { size: 0, storageSize: 0, avgObjSize: 0, totalIndexSize: 0, nindexes: 0 };
                
                // Try to get indexes count
                try {
                    const indexes = await coll.listIndexes().toArray();
                    stats.nindexes = indexes.length;
                } catch (indexError) {
                    console.warn(`Could not get indexes for ${collection.name}:`, indexError.message);
                }
                
                // Estimate size only for small collections
                if (count > 0 && count < 1000) {
                    try {
                        const sampleDocs = await coll.find({}).limit(10).toArray();
                        if (sampleDocs.length > 0) {
                            const avgSize = JSON.stringify(sampleDocs).length / sampleDocs.length;
                            stats.avgObjSize = avgSize;
                            stats.size = Math.round(avgSize * count);
                            stats.storageSize = stats.size;
                        }
                    } catch (sampleError) {
                        console.warn(`Could not sample ${collection.name}:`, sampleError.message);
                    }
                }
                
                collectionStats.push({
                    name: collection.name,
                    type: collection.name.startsWith('logs_') ? 'logs' : 'metadata',
                    cacheHash: collection.name.startsWith('logs_') ? collection.name.replace('logs_', '') : null,
                    count: count,
                    size: stats.size || 0,
                    storageSize: stats.storageSize || 0,
                    avgObjSize: stats.avgObjSize || 0,
                    indexSize: stats.totalIndexSize || 0,
                    indexes: stats.nindexes || 0
                });
            } catch (error) {
                // Some collections might not be accessible
                collectionStats.push({
                    name: collection.name,
                    type: collection.name.startsWith('logs_') ? 'logs' : 'metadata',
                    cacheHash: collection.name.startsWith('logs_') ? collection.name.replace('logs_', '') : null,
                    count: 0,
                    size: 0,
                    storageSize: 0,
                    avgObjSize: 0,
                    indexSize: 0,
                    indexes: 0,
                    error: error.message
                });
            }
        }
        
        res.json({
            collections: collectionStats,
            total: collections.length
        });
    } catch (error) {
        console.error('Error getting collections:', error);
        res.status(500).json({ error: 'Failed to get collections' });
    }
});

/**
 * Get detailed information about a specific log collection
 */
app.get('/api/mongodb/collection/:cacheHash', async (req, res) => {
    try {
        const { cacheHash } = req.params;
        const isAvailable = await preprocessor.mongoService.isAvailable();
        
        if (!isAvailable) {
            return res.status(503).json({ error: 'MongoDB is not available' });
        }
        
        const logCollection = preprocessor.mongoService.getLogCollection(cacheHash);
        
        // Get basic stats
        const count = await logCollection.countDocuments();
        
        // Try to get collection stats with fallback
        let stats = { size: 0, storageSize: 0, avgObjSize: 0, totalIndexSize: 0, nindexes: 0 };
        try {
            stats = await logCollection.stats();
        } catch (statsError) {
            // Fallback: estimate size based on sample documents
            try {
                const sampleDocs = await logCollection.find({}).limit(100).toArray();
                if (sampleDocs.length > 0) {
                    const avgSize = JSON.stringify(sampleDocs).length / sampleDocs.length;
                    stats.avgObjSize = avgSize;
                    stats.size = Math.round(avgSize * count);
                    stats.storageSize = stats.size;
                }
                
                const indexes = await logCollection.listIndexes().toArray();
                stats.nindexes = indexes.length;
            } catch (fallbackError) {
                console.warn(`Could not get stats for ${cacheHash}:`, fallbackError.message);
            }
        }
        
        // Get level distribution
        const levelStats = await logCollection.aggregate([
            { $group: { _id: '$level', count: { $sum: 1 } } },
            { $sort: { count: -1 } }
        ]).toArray();
        
        // Get target distribution
        const targetStats = await logCollection.aggregate([
            { $group: { _id: '$target', count: { $sum: 1 } } },
            { $sort: { count: -1 } },
            { $limit: 10 }
        ]).toArray();
        
        // Get panic count
        const panicCount = await logCollection.countDocuments({ '_meta.isPanic': true });
        
        // Get time range
        const timeRange = await logCollection.aggregate([
            {
                $group: {
                    _id: null,
                    minTime: { $min: '$timestampDate' },
                    maxTime: { $max: '$timestampDate' }
                }
            }
        ]).toArray();
        
        res.json({
            cacheHash,
            collectionName: `logs_${cacheHash}`,
            count,
            stats: {
                size: stats.size,
                storageSize: stats.storageSize,
                avgObjSize: stats.avgObjSize,
                indexSize: stats.totalIndexSize || 0,
                indexes: stats.nindexes || 0
            },
            distribution: {
                levels: levelStats.reduce((acc, item) => {
                    acc[item._id] = item.count;
                    return acc;
                }, {}),
                targets: targetStats.reduce((acc, item) => {
                    acc[item._id] = item.count;
                    return acc;
                }, {}),
                panicCount
            },
            timeRange: timeRange.length > 0 ? {
                start: timeRange[0].minTime,
                end: timeRange[0].maxTime
            } : null
        });
    } catch (error) {
        console.error('Error getting collection details:', error);
        res.status(500).json({ error: 'Failed to get collection details' });
    }
});

/**
 * Delete specific logs based on criteria
 */
app.delete('/api/mongodb/logs/:cacheHash', async (req, res) => {
    try {
        const { cacheHash } = req.params;
        const { filters } = req.body;
        
        const isAvailable = await preprocessor.mongoService.isAvailable();
        if (!isAvailable) {
            return res.status(503).json({ error: 'MongoDB is not available' });
        }
        
        const logCollection = preprocessor.mongoService.getLogCollection(cacheHash);
        const query = preprocessor.mongoService.buildQuery(filters || {});
        
        // Get count before deletion
        const countBefore = await logCollection.countDocuments(query);
        
        if (countBefore === 0) {
            return res.json({ message: 'No logs match the specified criteria', deletedCount: 0 });
        }
        
        // Delete matching logs
        const result = await logCollection.deleteMany(query);
        
        res.json({
            message: `Deleted ${result.deletedCount} logs from collection logs_${cacheHash}`,
            deletedCount: result.deletedCount,
            criteria: filters
        });
    } catch (error) {
        console.error('Error deleting logs:', error);
        res.status(500).json({ error: 'Failed to delete logs' });
    }
});

/**
 * Clear all MongoDB data (nuclear option)
 */
app.delete('/api/mongodb/clear-all', async (req, res) => {
    try {
        const isAvailable = await preprocessor.mongoService.isAvailable();
        if (!isAvailable) {
            return res.status(503).json({ error: 'MongoDB is not available' });
        }
        
        // Get list of collections before deletion
        const collections = await preprocessor.mongoService.db.listCollections().toArray();
        const collectionNames = collections.map(col => col.name);
        
        // Drop all collections
        for (const collection of collections) {
            try {
                await preprocessor.mongoService.db.collection(collection.name).drop();
            } catch (error) {
                // Collection might already be dropped, that's okay
                console.warn(`Failed to drop collection ${collection.name}:`, error.message);
            }
        }
        
        res.json({
            message: 'All MongoDB collections cleared successfully',
            deletedCollections: collectionNames,
            count: collectionNames.length
        });
    } catch (error) {
        console.error('Error clearing MongoDB:', error);
        res.status(500).json({ error: 'Failed to clear MongoDB' });
    }
});

/**
 * Optimize MongoDB (rebuild indexes)
 */
app.post('/api/mongodb/optimize', async (req, res) => {
    try {
        const isAvailable = await preprocessor.mongoService.isAvailable();
        if (!isAvailable) {
            return res.status(503).json({ error: 'MongoDB is not available' });
        }
        
        const collections = await preprocessor.mongoService.db.listCollections().toArray();
        const optimizationResults = [];
        
        for (const collection of collections) {
            try {
                if (collection.name.startsWith('logs_')) {
                    const coll = preprocessor.mongoService.db.collection(collection.name);
                    
                    // Recreate indexes
                    await preprocessor.mongoService.createIndexes(coll);
                    
                    optimizationResults.push({
                        collection: collection.name,
                        status: 'optimized',
                        action: 'indexes recreated'
                    });
                }
            } catch (error) {
                optimizationResults.push({
                    collection: collection.name,
                    status: 'error',
                    error: error.message
                });
            }
        }
        
        res.json({
            message: 'MongoDB optimization completed',
            results: optimizationResults
        });
    } catch (error) {
        console.error('Error optimizing MongoDB:', error);
        res.status(500).json({ error: 'Failed to optimize MongoDB' });
    }
});

// Error handling middleware
app.use((error, req, res, next) => {
    console.error('Unhandled error:', error);
    
    if (error instanceof multer.MulterError) {
        if (error.code === 'LIMIT_FILE_SIZE') {
            return res.status(400).json({ error: 'File too large' });
        }
    }
    
    res.status(500).json({ error: 'Internal server error' });
});

// Start server
app.listen(PORT, () => {
    console.log(`Ruse Viewer server running on port ${PORT}`);
    console.log(`Access the web interface at: http://localhost:${PORT}`);
});

module.exports = app;
