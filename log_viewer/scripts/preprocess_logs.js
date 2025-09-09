const fs = require('fs-extra');
const path = require('path');
const crypto = require('crypto');
const readline = require('readline');
const cliProgress = require('cli-progress');
const child_process = require('child_process');
const MongoDBService = require('./mongodb_service');

class LogPreprocessor {
    constructor() {
        this.mongoService = new MongoDBService();
    }

    async connectToMongo() {
        await this.mongoService.connect();
        await this.ensureMongoIsAvailable();
    }

    async disconnectFromMongo() {
        await this.mongoService.disconnect();
    }

    async ensureMongoIsAvailable() {
        if (!await this.mongoService.isAvailable()) {
            throw new Error('MongoDB is not available');
        }
    }

    /**
     * Generate cache filename based on log file path and result file path
     */
    getCacheFilename(logFilePath, resultFilePath) {
        const logStats = fs.statSync(logFilePath);
        const resultStats = fs.existsSync(resultFilePath) ? fs.statSync(resultFilePath) : { mtime: new Date(0) };
        
        const hash = crypto.createHash('md5')
            .update(logFilePath + logStats.mtime.getTime() + resultFilePath + resultStats.mtime.getTime())
            .digest('hex');
        return hash;
    }

    /**
     * Parse result file and extract metadata
     */
    async parseResultFile(runId, resultDirPath) {
        if (!fs.existsSync(resultDirPath)) {
            throw new Error(`Result directory not found: ${resultDirPath}`);
        }

        console.log(`Parsing result directory: ${resultDirPath}`);
        
        try {

            let raw_tasks = [];
            let raw_metadata = null;

            fs.readdirSync(resultDirPath).forEach(file => {
                const content = fs.readFileSync(path.join(resultDirPath, file), 'utf8');
                if (file === 'metadata.json') {
                    raw_metadata = JSON.parse(content);
                } else {
                    raw_tasks.push(JSON.parse(content));
                }
            });
            
            // Extract key information from result file
            const metadata = {
                timestamp: raw_metadata.timestamp,
                runTime: new Date(raw_metadata.timestamp * 1000).toISOString(),
                sysinfo: raw_metadata.sysinfo || {},
                config: raw_metadata.config || {},
                taskCount: raw_tasks.length,
                passedTasks: raw_tasks.filter(t => !t.error).length,
                failedTasks: raw_tasks.filter(t => t.error).length,
                totalTime: this.calculateTotalTime(raw_tasks || []),
                averageTime: this.calculateAverageTime(raw_tasks || []),
                maxDepth: this.calculateMaxDepth(raw_tasks || []),
                maxSize: this.calculateMaxSize(raw_tasks || [])
            };

            return {
                raw: {
                    "tasks": raw_tasks,
                    "metadata": raw_metadata
                },
                metadata: metadata
            };
        } catch (error) {
            throw new Error(`Failed to parse result file: ${error.message}`);
        }
    }

    /**
     * Calculate total time across all tasks
     */
    calculateTotalTime(tasks) {
        return tasks.reduce((total, task) => {
            if (task.total_time) {
                const seconds = task.total_time.secs || 0;
                const nanos = task.total_time.nanos || 0;
                return total + seconds + (nanos / 1000000000);
            }
            return total;
        }, 0);
    }

    /**
     * Calculate average time per task
     */
    calculateAverageTime(tasks) {
        if (tasks.length === 0) return 0;
        const total = this.calculateTotalTime(tasks);
        return total / tasks.length;
    }

    /**
     * Calculate maximum depth across all tasks
     */
    calculateMaxDepth(tasks) {
        return Math.max(...tasks.map(task => {
            if (task.total_statistics && task.total_statistics.MaxDepth) {
                return task.total_statistics.MaxDepth;
            }
            return 0;
        }), 0);
    }

    /**
     * Calculate maximum size across all tasks
     */
    calculateMaxSize(tasks) {
        return Math.max(...tasks.map(task => {
            if (task.total_statistics && task.total_statistics.MaxSize) {
                return task.total_statistics.MaxSize;
            }
            return 0;
        }), 0);
    }

    /**
     * Parse panic location string (filename:line_number)
     */
    parsePanicLocation(locationStr) {
        if (typeof locationStr !== 'string') return null;

        const lastColon = locationStr.lastIndexOf(':');
        if (lastColon === -1) return null;

        const filename = locationStr.substring(0, lastColon);
        const lineNumber = parseInt(locationStr.substring(lastColon + 1), 10);

        if (isNaN(lineNumber)) return null;

        return { filename, line_number: lineNumber };
    }

    /**
     * Filter backtrace to remove noise and highlight relevant frames
     */
    filterBacktrace(backtrace) {
        if (!backtrace || backtrace === 'disabled backtrace') {
            return { type: 'disabled', frames: [] };
        }

        const lines = backtrace.split('\n').filter(line => line.trim());
        const frames = [];

        for (const line of lines) {
            // Skip common noise patterns
            if (line.includes('rust_begin_unwind') ||
                line.includes('std::panic') ||
                line.includes('core::panic') ||
                line.includes('__rust_start_panic')) {
                continue;
            }

            // Extract meaningful frame information
            const frameMatch = line.match(/^\s*(\d+):\s*(.+?)(?:\s+at\s+(.+))?$/);
            if (frameMatch) {
                frames.push({
                    index: parseInt(frameMatch[1]),
                    symbol: frameMatch[2]?.trim(),
                    location: frameMatch[3]?.trim(),
                    raw: line.trim()
                });
            } else {
                // Keep unmatched lines as raw frames
                frames.push({
                    raw: line.trim()
                });
            }
        }

        return { type: 'full', frames };
    }

    /**
     * Extract special extensions from fields
     */
    extractExtensions(fields) {
        const extensions = {};

        for (const [key, value] of Object.entries(fields)) {
            const dotIndex = key.lastIndexOf('.');
            if (dotIndex !== -1) {
                const extension = key.substring(dotIndex + 1);
                const baseKey = key.substring(0, dotIndex);

                if (!extensions[extension]) {
                    extensions[extension] = {};
                }

                // Handle .json extension recursively
                if (extension === 'json') {
                    try {
                        // Parse the JSON value
                        const parsedJson = typeof value === 'string' ? JSON.parse(value) : value;
                        
                        // Recursively extract extensions from the parsed JSON
                        const nestedExtensions = this.extractExtensionsFromNestedObject(parsedJson);
                        
                        // Store both the raw JSON and any nested extensions
                        extensions[extension][baseKey] = {
                            raw: parsedJson,
                            extensions: nestedExtensions
                        };
                    } catch (error) {
                        // If JSON parsing fails, store as regular value
                        console.warn(`Failed to parse JSON for key ${key}:`, error.message);
                        extensions[extension][baseKey] = value;
                    }
                } else {
                    // Handle other extensions normally
                    extensions[extension][baseKey] = value;
                }
            }
        }

        return extensions;
    }

    /**
     * Extract extensions from a nested object structure (for JSON extensions)
     */
    extractExtensionsFromNestedObject(obj, keyPrefix = '') {
        const extensions = {};

        if (typeof obj !== 'object' || obj === null) {
            return extensions;
        }

        for (const [key, value] of Object.entries(obj)) {
            const fullKey = keyPrefix ? `${keyPrefix}.${key}` : key;
            
            // Check if this key has an extension
            const dotIndex = key.lastIndexOf('.');
            if (dotIndex !== -1) {
                const extension = key.substring(dotIndex + 1);
                const baseKey = key.substring(0, dotIndex);
                const fullBaseKey = keyPrefix ? `${keyPrefix}.${baseKey}` : baseKey;

                if (!extensions[extension]) {
                    extensions[extension] = {};
                }

                // Handle nested .json extension
                if (extension === 'json') {
                    try {
                        const parsedJson = typeof value === 'string' ? JSON.parse(value) : value;
                        const nestedExtensions = this.extractExtensionsFromNestedObject(parsedJson, fullKey);
                        
                        extensions[extension][fullBaseKey] = {
                            raw: parsedJson,
                            extensions: nestedExtensions
                        };
                    } catch (error) {
                        console.warn(`Failed to parse nested JSON for key ${fullKey}:`, error.message);
                        extensions[extension][fullBaseKey] = value;
                    }
                } else {
                    // Handle other extensions (like .mermaid)
                    extensions[extension][fullBaseKey] = value;
                }
            } else if (typeof value === 'object' && value !== null) {
                // Recursively process nested objects
                const nestedExtensions = this.extractExtensionsFromNestedObject(value, fullKey);
                
                // Merge nested extensions into the main extensions object
                for (const [extType, extData] of Object.entries(nestedExtensions)) {
                    if (!extensions[extType]) {
                        extensions[extType] = {};
                    }
                    Object.assign(extensions[extType], extData);
                }
            }
        }

        return extensions;
    }

    /**
     * Process a single log entry
     */
    processLogEntry(entry, lineNumber) {
        try {
            const processed = {
                ...entry,
                _meta: {
                    lineNumber,
                    originalFilename: entry.filename,
                    originalLineNumber: entry.line_number,
                    extensions: {},
                    span: {},
                    isPanic: false
                }
            };

            // Check for panic-specific fields
            if (entry.fields && entry.fields['panic.location']) {
                processed._meta.isPanic = true;

                const panicLocation = this.parsePanicLocation(entry.fields['panic.location']);
                if (panicLocation) {
                    processed.filename = panicLocation.filename;
                    processed.line_number = panicLocation.line_number;
                }

                if (entry.fields['panic.backtrace']) {
                    processed._meta.backtrace = this.filterBacktrace(entry.fields['panic.backtrace']);
                }
            }

            if (entry.spans && Array.isArray(entry.spans)) {
                processed._meta.span.name = entry.spans.map(s => s.name).join(':');
                processed._meta.span.task = entry.spans.find(s => s.task_name !== undefined)?.task_name;
                processed._meta.span.iteration = entry.spans.find(s => s.iteration !== undefined)?.iteration;
                processed._meta.span.worker = entry.spans.find(s => s.worker_index !== undefined)?.worker_index;
                processed._meta.span.arg_types = entry.spans.find(s => s.arg_types !== undefined)?.arg_types;
                processed._meta.span.ops = entry.spans.find(s => s.ops !== undefined)?.ops;
            }

            // Extract extensions
            if (entry.fields) {
                processed._meta.extensions = this.extractExtensions(entry.fields);
            }

            return processed;
        } catch (error) {
            console.warn(`Error processing log entry at line ${lineNumber}:`, error.message);
            return {
                ...entry,
                _meta: {
                    lineNumber,
                    error: error.message,
                    extensions: {},
                    span: {},
                    isPanic: false
                }
            };
        }
    }

    /**
     * Process log file and return processed entries
     */
    async processLogFile(runId, logFilePath) {
        console.log(`Processing log file: ${logFilePath}`);

        // Get file size for progress tracking
        const stats = fs.statSync(logFilePath);
        const fileSize = stats.size;
        const lines = child_process.execSync(`wc -l ${logFilePath}`).toString().trim().split(' ')[0];

        // Initialize MongoDB storage if available
        try {
            await this.mongoService.initializeLogStorage(runId);
            console.log('Initialized MongoDB storage for streaming...');
        } catch (error) {
            console.warn(`Failed to initialize MongoDB storage: ${error.message}`);
            throw new Error('Failed to initialize MongoDB storage');
        }

        // Create progress bar
        const progressBar = new cliProgress.SingleBar({
            format: 'Processing |{bar}| {percentage}% | {value}/{total} lines | ETA: {eta_formatted} | {validLogs} valid logs',
            barCompleteChar: '\u2588',
            barIncompleteChar: '\u2591',
            hideCursor: true,
            etaBuffer: Math.max(Math.floor(lines / 500), 10)
        });

        console.log(`File size: ${(fileSize / 1024 / 1024).toFixed(2)} MB`);
        console.log(`Log lines: ${lines}`);
        progressBar.start(lines, 0, { validLogs: 0 });

        // Create read stream and process line by line
        const fileStream = fs.createReadStream(logFilePath, { encoding: 'utf8' });
        const rl = readline.createInterface({
            input: fileStream,
            crlfDelay: Infinity // Handle Windows line endings
        });

        const metadata = {
            sourceFile: logFilePath,
            processedAt: new Date().toISOString(),
            totalLines: 0,
            stats: {
                levels: {},
                targets: {},
                threadIds: {},
                panicCount: 0,
                tasks: {}
            }
        };

        let validLogCount = 0;
        let lineNumber = 0;
        const processedLogs = []; // For MongoDB storage - batch processing
        const MONGODB_BATCH_SIZE = 1000; // Process in smaller batches

        // Function to process batch and store in MongoDB
        const processBatch = async (batch) => {
            if (batch.length > 0) {
                try {
                    await this.mongoService.storeLogs(runId, batch, metadata);
                } catch (error) {
                    console.warn(`Failed to store batch to MongoDB: ${error.message}`);
                }
            }
        };

        for await (const line of rl) {
            lineNumber++;
            const trimmedLine = line.trim();

            if (!trimmedLine) continue;

            try {
                const entry = JSON.parse(trimmedLine);
                const processedEntry = this.processLogEntry(entry, lineNumber);

                // Store for MongoDB - batch processing
                processedLogs.push(processedEntry);
                // Process batch when it reaches the batch size
                if (processedLogs.length >= MONGODB_BATCH_SIZE) {
                    await processBatch([...processedLogs]); // Copy array
                    processedLogs.length = 0; // Clear the array
                }

                validLogCount++;

                // Update statistics
                const level = entry.level || 'unknown';
                const target = entry.target || 'unknown';
                const threadId = entry.threadId || 'unknown';

                metadata.stats.levels[level] = (metadata.stats.levels[level] || 0) + 1;
                metadata.stats.targets[target] = (metadata.stats.targets[target] || 0) + 1;
                metadata.stats.threadIds[threadId] = (metadata.stats.threadIds[threadId] || 0) + 1;

                if (processedEntry._meta.isPanic) {
                    metadata.stats.panicCount++;
                }

                if (processedEntry._meta.span.task) {
                    const taskName = processedEntry._meta.span.task;
                    if (!metadata.stats.tasks[taskName]) {
                        metadata.stats.tasks[taskName] = {
                            count: 0,
                            metadata_logs: 0,
                            iterations: {}
                        }
                    }
                    metadata.stats.tasks[taskName].count++;
                    if (processedEntry._meta.span.iteration !== undefined) {
                        const iteration = processedEntry._meta.span.iteration;
                        metadata.stats.tasks[taskName].iterations[iteration] = (metadata.stats.tasks[taskName].iterations[iteration] || 0) + 1;
                    } else {
                        // Task exists but no iteration - categorize as "no-iteration"
                        metadata.stats.tasks[taskName].metadata_logs++;
                    }
                } else {
                    // Logs without tasks are categorized as "metadata"
                    const taskName = "metadata";
                    if (!metadata.stats.tasks[taskName]) {
                        metadata.stats.tasks[taskName] = {
                            count: 0,
                            iterations: {}
                        }
                    }
                    metadata.stats.tasks[taskName].count++;
                }
                progressBar.increment(1, { validLogs: validLogCount });
            } catch (error) {
                console.warn(`Failed to parse line ${lineNumber}: ${error.message}`);
            }
        }

        // Complete progress bar
        progressBar.stop();

        // Update metadata with final counts
        metadata.totalLines = lineNumber;
        metadata.validLogCount = validLogCount;

        // Process any remaining logs in the batch
        if (processedLogs.length > 0) {
            console.log('\nStoring final batch in MongoDB...');
            try {
                await processBatch(processedLogs);
                console.log(`Successfully stored final batch of ${processedLogs.length} logs in MongoDB`);
            } catch (error) {
                console.error('Failed to store final batch in MongoDB:', error.message);
                console.log('Continuing with file-based storage...');
            }
        }

        console.log(`\nProcessed ${validLogCount} logs from ${lineNumber} total lines`);
        console.log(`Logs stored in MongoDB for fast querying`);

        // Store final metadata in MongoDB
        try {
            await this.mongoService.storeMetadata(runId, metadata);
            console.log('Stored metadata in MongoDB');
        } catch (error) {
            console.warn('Failed to store metadata in MongoDB:', error.message);
        }

        return {
            metadata: metadata,
            runId: runId,
            logs: [] // Return empty logs array since we're streaming - caller should read from cache file if needed
        };
    }

    /**
     * Process a complete Ruse run (log file + result file)
     */
    async processRuseRun(logFilePath, resultFilePath) {
        console.log(`Processing Ruse run:`);
        console.log(`  Log file: ${logFilePath}`);
        console.log(`  Result file: ${resultFilePath}`);

        const runId = this.getCacheFilename(logFilePath, resultFilePath);

        // Check MongoDB first
        try {
            const metadata = await this.mongoService.getMetadata(runId);
            console.log(`Loading from MongoDB cache: ${runId}`);
            return {
                metadata: metadata,
                runId: runId,
                logs: [] // Don't load logs into memory
            };
        } catch (error) {
            console.log(`MongoDB cache not found...`);
        }

        // Initialize MongoDB storage if available
        try {
            await this.mongoService.initializeLogStorage(runId);
            console.log('Initialized MongoDB storage for streaming...');
        } catch (error) {
            console.warn(`Failed to initialize MongoDB storage: ${error.message}`);
            throw new Error('Failed to initialize MongoDB storage');
        }

        // Parse result file first
        const resultData = await this.parseResultFile(runId, resultFilePath);
        
        // Process log file
        const logData = await this.processLogFile(runId,logFilePath);
        
        // Create combined metadata for the Ruse run
        const runMetadata = {
            ...logData.metadata,
            resultFile: resultFilePath,
            resultMetadata: resultData.metadata,
            resultData: resultData.raw,
            runId: runId,
            runTime: resultData.metadata.runTime,
            timestamp: resultData.metadata.timestamp
        };

        // Store final metadata in MongoDB
        try {
            await this.mongoService.storeMetadata(runId, runMetadata);
            console.log('Stored Ruse run metadata in MongoDB');
        } catch (error) {
            console.warn('Failed to store metadata in MongoDB:', error.message);
        }

        return {
            metadata: runMetadata,
            runId: runId,
            logs: [] // Return empty logs array since we're streaming
        };
    }

    /**
     * List available cached files
     */
    async listCached() {
        return await this.mongoService.listCached();
    }

    /**
     * Read logs from cache with pagination
     */
    async readLogsFromCache(cacheHash, page = 1, limit = 100, filters = {}) {
        return await this.mongoService.queryLogs(cacheHash, filters, page, limit);
    }

    /**
     * Count total logs matching filters
     */
    async countLogsWithFilters(cacheHash, filters = {}) {
        const result = await this.mongoService.queryLogs(cacheHash, filters, 1, 1);
        return result.pagination.total;
    }

    /**
     * Check if a log entry passes the given filters
     */
    passesFilters(logEntry, filters) {
        // Level filter
        if (filters.level && Array.isArray(filters.level)) {
            if (!filters.level.includes(logEntry.level)) {
                return false;
            }
        }

        // Target filter
        if (filters.target && Array.isArray(filters.target)) {
            if (!filters.target.includes(logEntry.target)) {
                return false;
            }
        }

        // Search filter
        if (filters.search) {
            const searchLower = filters.search.toLowerCase();
            let found = false;

            if (logEntry.fields && logEntry.fields.message &&
                logEntry.fields.message.toLowerCase().includes(searchLower)) {
                found = true;
            }

            if (!found && logEntry.fields) {
                for (const value of Object.values(logEntry.fields)) {
                    if (typeof value === 'string' &&
                        value.toLowerCase().includes(searchLower)) {
                        found = true;
                        break;
                    }
                }
            }

            if (!found) return false;
        }

        // Panic filter
        if (filters.isPanic === 'true') {
            if (!logEntry._meta || !logEntry._meta.isPanic) {
                return false;
            }
        } else if (filters.isPanic === 'false') {
            if (logEntry._meta && logEntry._meta.isPanic) {
                return false;
            }
        }

        return true;
    }
}

// CLI usage
if (require.main === module) {
    const preprocessor = new LogPreprocessor();

    const logFilePath = process.argv[2];
    const resultFilePath = process.argv[3];
    
    if (!logFilePath || !resultFilePath) {
        console.error('Usage: node preprocess_logs.js <log_file_path> <result_file_path>');
        console.error('Both log file and result file are required for a complete Ruse run.');
        process.exit(1);
    }

    if (!fs.existsSync(logFilePath)) {
        console.error(`Log file not found: ${logFilePath}`);
        process.exit(1);
    }

    if (!fs.existsSync(resultFilePath)) {
        console.error(`Result file not found: ${resultFilePath}`);
        process.exit(1);
    }

    // Process as complete Ruse run
    preprocessor.processRuseRun(logFilePath, resultFilePath)
        .then(result => {
            console.log('\nProcessing completed!');
            console.log(`Ruse run processed successfully!`);
            console.log(`Run time: ${result.metadata.runTime}`);
            console.log(`Total tasks: ${result.metadata.resultMetadata.taskCount}`);
            console.log(`Passed tasks: ${result.metadata.resultMetadata.passedTasks}`);
            console.log(`Failed tasks: ${result.metadata.resultMetadata.failedTasks}`);
            console.log(`Total time: ${result.metadata.resultMetadata.totalTime.toFixed(2)}s`);
        })
        .catch(error => {
            console.error('Processing failed:', error);
            process.exit(1);
        })
        .finally(() => {
            preprocessor.disconnectFromMongo();
        });
}

module.exports = LogPreprocessor;
