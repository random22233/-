package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"log"
	"os"
	"sync"
	"time"

	"github.com/gagliardetto/solana-go"
	"github.com/gagliardetto/solana-go/rpc"
	"github.com/spf13/viper"
)

type Config struct {
	RpcURL    string                `mapstructure:"rpc_url"`
	Transfers []TransferInstruction `mapstructure:"transfers"`
}

type TransferInstruction struct {
	FromPrivateKey string `mapstructure:"from_private_key"`
	ToAddress      string `mapstructure:"to_address"`
	Amount         uint64 `mapstructure:"amount"`
}

type TransferResult struct {
	FromAccount    string
	ToAccount      string
	Amount         uint64
	Signature      string
	Status         string
	ProcessingTime time.Duration
	Error          error
}

func loadConfig() (*Config, error) {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")

	if err := viper.ReadInConfig(); err != nil {
		return nil, fmt.Errorf("error reading config file: %w", err)
	}

	var config Config
	if err := viper.Unmarshal(&config); err != nil {
		return nil, fmt.Errorf("unable to decode config into struct: %w", err)
	}

	return &config, nil
}

func executeTransfer(client *rpc.Client, transfer TransferInstruction, wg *sync.WaitGroup, results chan<- TransferResult) {
	defer wg.Done()

	result := TransferResult{
		Amount: transfer.Amount,
	}

	startTime := time.Now()

	// Decode private key
	privateKeyBytes, err := base64.StdEncoding.DecodeString(transfer.FromPrivateKey)
	if err != nil {
		result.Error = fmt.Errorf("failed to decode private key: %w", err)
		results <- result
		return
	}

	// Create account from private key
	account := solana.NewAccountFromPrivateKeyBytes(privateKeyBytes)
	result.FromAccount = account.PublicKey().String()

	// Parse destination address
	destination, err := solana.PublicKeyFromBase58(transfer.ToAddress)
	if err != nil {
		result.Error = fmt.Errorf("invalid destination address: %w", err)
		results <- result
		return
	}
	result.ToAccount = destination.String()

	// Get recent blockhash
	recentBlockhash, err := client.GetRecentBlockhash(context.Background(), rpc.CommitmentFinalized)
	if err != nil {
		result.Error = fmt.Errorf("failed to get recent blockhash: %w", err)
		results <- result
		return
	}

	// Create transfer instruction
	instruction := solana.NewTransferInstruction(
		transfer.Amount,
		account.PublicKey(),
		destination,
	).Build()

	// Create transaction
	tx, err := solana.NewTransaction(
		[]solana.Instruction{instruction},
		recentBlockhash.Value.Blockhash,
		solana.TransactionPayer(account.PublicKey()),
	)
	if err != nil {
		result.Error = fmt.Errorf("failed to create transaction: %w", err)
		results <- result
		return
	}

	// Sign transaction
	_, err = tx.Sign(
		func(key solana.PublicKey) *solana.PrivateKey {
			if account.PublicKey().Equals(key) {
				return &account.PrivateKey
			}
			return nil
		},
	)
	if err != nil {
		result.Error = fmt.Errorf("failed to sign transaction: %w", err)
		results <- result
		return
	}

	// Send transaction
	sig, err := client.SendTransactionWithOpts(
		context.Background(),
		tx,
		rpc.TransactionOpts{
			SkipPreflight:       false,
			PreflightCommitment: rpc.CommitmentFinalized,
		},
	)
	if err != nil {
		result.Error = fmt.Errorf("failed to send transaction: %w", err)
		results <- result
		return
	}
	result.Signature = sig.String()

	// Check transaction status
	for {
		status, err := client.GetSignatureStatuses(
			context.Background(),
			[]solana.Signature{sig},
		)
		if err != nil {
			result.Error = fmt.Errorf("failed to get transaction status: %w", err)
			results <- result
			return
		}

		if status.Value[0] != nil {
			if status.Value[0].Err != nil {
				result.Status = "Failed"
				result.Error = fmt.Errorf("transaction failed: %v", status.Value[0].Err)
			} else {
				result.Status = "Confirmed"
			}
			break
		}

		// Wait a bit before checking again
		time.Sleep(500 * time.Millisecond)
	}

	result.ProcessingTime = time.Since(startTime)
	results <- result
}

func main() {
	// Load configuration
	config, err := loadConfig()
	if err != nil {
		log.Fatalf("Failed to load configuration: %v", err)
	}

	// Create RPC client
	client := rpc.New(config.RpcURL)

	// Create a wait group to wait for all transfers to complete
	var wg sync.WaitGroup
	results := make(chan TransferResult, len(config.Transfers))

	// Start time measurement
	startTime := time.Now()

	fmt.Printf("Starting bulk transfer of %d transactions...\n", len(config.Transfers))

	// Execute transfers in parallel
	for _, transfer := range config.Transfers {
		wg.Add(1)
		go executeTransfer(client, transfer, &wg, results)
	}

	// Wait for all transfers to complete in a separate goroutine
	go func() {
		wg.Wait()
		close(results)
	}()

	// Collect results
	var successCount, failCount int
	var totalProcessingTime time.Duration
	var minTime, maxTime time.Duration
	var allResults []TransferResult

	fmt.Println("\nTransaction Results:")
	fmt.Println("====================")

	for result := range results {
		allResults = append(allResults, result)

		// Initialize minTime with the first result
		if minTime == 0 {
			minTime = result.ProcessingTime
		}

		// Update min and max times
		if result.ProcessingTime < minTime {
			minTime = result.ProcessingTime
		}
		if result.ProcessingTime > maxTime {
			maxTime = result.ProcessingTime
		}

		totalProcessingTime += result.ProcessingTime

		if result.Error != nil {
			failCount++
			fmt.Printf("❌ From: %s\n   To: %s\n   Amount: %d lamports\n   Error: %v\n\n", 
				result.FromAccount, result.ToAccount, result.Amount, result.Error)
		} else {
			successCount++
			fmt.Printf("✅ From: %s\n   To: %s\n   Amount: %d lamports\n   Signature: %s\n   Processing Time: %v\n\n", 
				result.FromAccount, result.ToAccount, result.Amount, result.Signature, result.ProcessingTime)
		}
	}

	// Calculate total time
	totalTime := time.Since(startTime)
	avgProcessingTime := totalProcessingTime / time.Duration(len(config.Transfers))

	// Print statistics
	fmt.Println("\nTransaction Statistics:")
	fmt.Println("======================")
	fmt.Printf("Total Transactions: %d\n", len(config.Transfers))
	fmt.Printf("Successful: %d\n", successCount)
	fmt.Printf("Failed: %d\n", failCount)
	fmt.Printf("Total Time: %v\n", totalTime)
	fmt.Printf("Minimum Processing Time: %v\n", minTime)
	fmt.Printf("Maximum Processing Time: %v\n", maxTime)
	fmt.Printf("Average Processing Time: %v\n", avgProcessingTime)

	// Exit with error if any transaction failed
	if failCount > 0 {
		os.Exit(1)
	}
}
